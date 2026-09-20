use super::schema::ScoreV1;

pub(super) fn subtract_score(left: &ScoreV1, right: &ScoreV1) -> Result<ScoreV1, String> {
    let (left_sign, mut left_digits, left_scale) = decimal_parts(left.as_str());
    let (right_sign, mut right_digits, right_scale) = decimal_parts(right.as_str());
    let scale = left_scale.max(right_scale);
    left_digits.extend(std::iter::repeat_n(0, scale - left_scale));
    right_digits.extend(std::iter::repeat_n(0, scale - right_scale));
    let (sign, digits) = signed_add(left_sign, left_digits, -right_sign, right_digits);
    ScoreV1::new(format_decimal(sign, digits, scale)).map_err(|error| error.to_string())
}

fn decimal_parts(value: &str) -> (i8, Vec<u8>, usize) {
    let sign = if value.starts_with('-') { -1 } else { 1 };
    let unsigned = value.trim_start_matches('-');
    let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let digits = integer
        .bytes()
        .chain(fraction.bytes())
        .map(|byte| byte - b'0')
        .collect();
    (sign, digits, fraction.len())
}

fn signed_add(a_sign: i8, a: Vec<u8>, b_sign: i8, b: Vec<u8>) -> (i8, Vec<u8>) {
    let width = a.len().max(b.len());
    let mut a = left_pad(a, width);
    let b = left_pad(b, width);
    if a_sign == b_sign {
        let mut carry = 0;
        for index in (0..width).rev() {
            let sum = a[index] + b[index] + carry;
            a[index] = sum % 10;
            carry = sum / 10;
        }
        if carry > 0 {
            a.insert(0, carry);
        }
        return (a_sign, a);
    }
    let (sign, mut high, low) = if a >= b {
        (a_sign, a, b)
    } else {
        (b_sign, b, a)
    };
    let mut borrow = 0i8;
    for index in (0..width).rev() {
        let value = high[index] as i8 - low[index] as i8 - borrow;
        high[index] = value.rem_euclid(10) as u8;
        borrow = i8::from(value < 0);
    }
    (sign, high)
}

fn left_pad(mut digits: Vec<u8>, width: usize) -> Vec<u8> {
    if digits.len() < width {
        let mut padded = vec![0; width - digits.len()];
        padded.append(&mut digits);
        padded
    } else {
        digits
    }
}

fn format_decimal(sign: i8, digits: Vec<u8>, scale: usize) -> String {
    let Some(first) = digits.iter().position(|digit| *digit != 0) else {
        return "0".into();
    };
    let mut raw = digits[first..]
        .iter()
        .map(|digit| char::from(b'0' + *digit))
        .collect::<String>();
    if raw.len() <= scale {
        raw.insert_str(0, &"0".repeat(scale + 1 - raw.len()));
    }
    if scale > 0 {
        raw.insert(raw.len() - scale, '.');
        while raw.ends_with('0') {
            raw.pop();
        }
        if raw.ends_with('.') {
            raw.pop();
        }
    }
    if sign < 0 {
        raw.insert(0, '-');
    }
    raw
}
