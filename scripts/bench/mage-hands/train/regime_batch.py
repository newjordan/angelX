"""CPU-testable minibatch/loss helpers. No model, provider, or CUDA setup."""


def collate_samples(samples, pad_token_id):
    import torch
    if not samples:
        raise ValueError('empty minibatch')
    for s in samples:
        ids = s['input_ids']
        if ids.ndim != 2 or ids.shape[0] != 1:
            raise ValueError('each sample must have one input_ids sequence')
        if not isinstance(s['n_ans'], int) or not 0 < s['n_ans'] < ids.shape[1]:
            raise ValueError('invalid answer-only loss mask')
        grid = s['image_grid_thw']
        if grid.ndim != 2 or grid.shape[1] != 3 or not bool((grid > 0).all()):
            raise ValueError('invalid image patch grid')
        patches = int(grid.prod(-1).sum())
        if (s['pixel_values'].ndim != 2 or s['pixel_values'].shape[0] != patches
                or s['patch_positions'].shape != (patches, 3)):
            raise ValueError('image patch counts disagree')
    width = max(s['input_ids'].shape[1] for s in samples)
    ids = torch.full((len(samples), width), pad_token_id, dtype=torch.long)
    labels = torch.full_like(ids, -100)
    mask = torch.zeros_like(ids)
    for i, s in enumerate(samples):
        n, a = s['input_ids'].shape[1], s['n_ans']
        ids[i, :n] = s['input_ids'][0]
        labels[i, n-a:n] = s['input_ids'][0, -a:]
        mask[i, :n] = 1
    return {'input_ids': ids, 'labels': labels, 'attention_mask': mask,
            **{k: torch.cat([s[k] for s in samples], dim=0)
               for k in ('pixel_values', 'image_grid_thw', 'patch_positions')}}


def answer_loss_sum(hidden, labels, lm_head):
    """Predict only answer tokens at their causal predecessors, never pad/prompt."""
    import torch.nn.functional as F
    if hidden.shape[:2] != labels.shape:
        raise ValueError('hidden/label sequence shapes disagree')
    shifted = labels[:, 1:]
    mask = shifted != -100
    tokens = int(mask.sum())
    if tokens == 0:
        raise ValueError('no answer tokens')
    # Avoid projecting the thousands of image/prompt tokens into the vocabulary.
    logits = lm_head(hidden[:, :-1][mask]).float()
    return F.cross_entropy(logits, shifted[mask], reduction='sum'), tokens


def finish_accumulation(optimizer, params, answer_tokens, max_grad_norm=1.0):
    """Apply a token-mean update, including a short last window; never discard it."""
    import torch
    if answer_tokens <= 0:
        raise ValueError('answer_tokens must be positive')
    params = list(params)
    if not any(p.grad is not None for p in params):
        raise ValueError('no gradients to update')
    for p in params:
        if p.grad is not None:
            p.grad.div_(answer_tokens)
    norm = torch.nn.utils.clip_grad_norm_(params, max_grad_norm)
    if not bool(torch.isfinite(norm)):
        optimizer.zero_grad(set_to_none=True)
        raise ValueError('non-finite training gradients')
    optimizer.step()
    optimizer.zero_grad(set_to_none=True)
    return float(norm)


def train_epoch(batches, loss_on, optimizer, params, accumulation_steps, deadline,
                log, max_grad_norm=1.0, clock=None, before_step=None):
    """One pass of genuine minibatches, with a token-normalized short final step.

    loss_on returns (summed causal answer loss, number of answer tokens).
    A normal deadline/exhaustion flushes pending work; an error aborts, not steps.
    """
    import time
    import torch
    if type(accumulation_steps) is not int or accumulation_steps < 1:
        raise ValueError('accumulation_steps must be a positive integer')
    clock = clock or time.monotonic
    params = list(params)
    start = clock()
    steps = samples = tokens = microbatches = 0
    window_tokens = window_samples = window_batches = 0
    window_loss = 0.0
    optimizer.zero_grad(set_to_none=True)

    def flush():
        nonlocal steps, window_tokens, window_samples, window_batches, window_loss
        if not window_tokens:
            return
        if before_step is not None:
            before_step(steps)
        norm = finish_accumulation(optimizer, params, window_tokens, max_grad_norm)
        steps += 1
        elapsed = max(clock() - start, 1e-9)
        log({'step': steps, 'samples': samples, 'answer_tokens': tokens,
             'microbatches': microbatches, 'window_samples': window_samples,
             'window_microbatches': window_batches, 'window_answer_tokens': window_tokens,
             'loss': window_loss / window_tokens, 'grad_norm': norm,
             'elapsed': elapsed, 'samples_per_second': samples / elapsed})
        window_tokens = window_samples = window_batches = 0
        window_loss = 0.0

    iterator = iter(batches)
    try:
        while clock() < deadline:
            try:
                batch = next(iterator)
            except StopIteration:
                break
            if clock() >= deadline:
                break
            loss, n_tokens = loss_on(batch)
            n_samples = int(batch['input_ids'].shape[0])
            if (type(n_tokens) is not int or n_tokens < 1 or n_samples < 1
                    or loss.numel() != 1 or not bool(torch.isfinite(loss))):
                raise ValueError('invalid or non-finite training loss')
            loss.backward()
            samples += n_samples
            tokens += n_tokens
            microbatches += 1
            window_samples += n_samples
            window_tokens += n_tokens
            window_batches += 1
            window_loss += float(loss.detach())
            if window_batches == accumulation_steps:
                flush()
        flush()
    except Exception:
        optimizer.zero_grad(set_to_none=True)
        raise
    return {'steps': steps, 'samples': samples, 'answer_tokens': tokens,
            'microbatches': microbatches, 'elapsed': clock() - start}
