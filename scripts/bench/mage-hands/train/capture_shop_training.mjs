// Capture actual local fake-shop transitions for hands-only labels. No model calls.
// No public-site navigation. Fetch interception rejects non-fixture requests.
// node capture_shop_training.mjs NEW_DIRECTORY [train|heldout]
import {spawn} from 'node:child_process';
import {existsSync, mkdirSync, writeFileSync} from 'node:fs';
import {resolve, join} from 'node:path';
import {createHash} from 'node:crypto';
import {setTimeout as sleep} from 'node:timers/promises';

const output=process.argv[2] && resolve(process.argv[2]);
const mode=process.argv[3]||'train';
if(!['train','heldout'].includes(mode))throw Error('mode must be train or heldout');
if(!output || existsSync(output))throw Error('A new immutable output directory is required');
mkdirSync(join(output,'shots'),{recursive:true});
const hash=s=>createHash('sha256').update(s).digest('hex');
const splitFor=g=>{const b=parseInt(hash('mage-regime-v2:'+g).slice(0,8),16)%10;return b<7?'train':b<9?'validation':'test';};
// Same predeclared combinations and deterministic splits in both modes.
// Heldout collection never relabels a workflow or adds it to training.
const scenarios=[];
for(const item of ['blue-l-shirt','silver-mug','cedar-bag']) {
  for(const qty of [2,3,4])for(const shipping of ['standard','express']) {
    const group=`shopping-transition:recovery-${item}-${qty}-${shipping}-remove-distractor-edit-shipping`;
    if((splitFor(group)==='train')===(mode==='train'))scenarios.push({item,qty,shipping,group});
  }
}
const widths=[1920,1600,1366];
const heights=[1080,1000,900];
writeFileSync(join(output,'plan.json'),JSON.stringify({scope:'local-fake-shop-only',role:'Mage hands only',mode,
  scenarios,viewports:widths.map((width,i)=>({width,height:heights[i]})),
  claim:'scripted transitions, not autonomous task success',heldout_assignments_changed:false},null,2));
let chromeLog='',fixture,ws,origin='',allDone=false;
const chrome=spawn(process.env.MAGE_CHROME||'chromium',['--headless=new','--remote-debugging-port=0',
  `--user-data-dir=${output}/chrome-profile`,'--no-first-run','--no-default-browser-check',
  '--disable-background-networking','--disable-gpu','--disable-sync','--disable-component-update',
  '--no-proxy-server','--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE localhost, EXCLUDE 127.0.0.1',
  '--hide-scrollbars','about:blank'],
  {stdio:['ignore','ignore','pipe']});
chrome.stderr.on('data',b=>chromeLog+=b);
const pages=[],transitions=[],oracles=[],failures=[];
const stopped=child=>!child||child.exitCode!==null||child.signalCode!==null;
async function stop(child){
  if(stopped(child))return;
  child.kill('SIGTERM');
  for(let i=0;i<40&&!stopped(child);i++)await sleep(50);
  if(!stopped(child)){child.kill('SIGKILL');for(let i=0;i<40&&!stopped(child);i++)await sleep(50);}
}
try {
  for(let i=0;i<150;i++) {
    const c=chromeLog.match(/DevTools listening on (ws:\/\/[^\s]+)/);
    if(c){const tabs=await(await fetch(`http://${new URL(c[1]).host}/json/list`)).json();
      const p=tabs.find(t=>t.type==='page');if(p){ws=new WebSocket(p.webSocketDebuggerUrl);break;}}
    if(chrome.exitCode!==null)throw Error('Isolated Chromium exited: '+chromeLog.slice(-1200));
    await sleep(100);
  }
  if(!ws)throw Error('Isolated Chromium not ready');
  await new Promise((ok,no)=>{ws.addEventListener('open',ok,{once:true});ws.addEventListener('error',no,{once:true});});
  let seq=0;const pending=new Map();
  const cdp=(method,params={})=>new Promise((ok,no)=>{
    const id=++seq,timer=setTimeout(()=>{pending.delete(id);no(Error(method+' timed out'));},20000);
    pending.set(id,{ok,no,timer});ws.send(JSON.stringify({id,method,params}));
  });
  ws.addEventListener('message',event=>{
    const j=JSON.parse(event.data);
    if(pending.has(j.id)){const p=pending.get(j.id);clearTimeout(p.timer);pending.delete(j.id);j.error?p.no(Error(JSON.stringify(j.error))):p.ok(j.result);}
    if(j.method==='Fetch.requestPaused') {
      const {requestId,request}=j.params;
      const allowed=origin && request.url.startsWith(origin+'/');
      cdp(allowed?'Fetch.continueRequest':'Fetch.failRequest',allowed?{requestId}:{requestId,errorReason:'BlockedByClient'}).catch(()=>{});
    }
  });
  const js=async expression=>{const r=await cdp('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});
    if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result?.value;};
  await cdp('Page.enable');await cdp('Fetch.enable',{patterns:[{urlPattern:'*'}]});
  async function settle(){await sleep(80);for(let i=0;i<60;i++){if(await js('document.readyState')==='complete')break;await sleep(50);}await js('document.fonts.ready.then(()=>true)');}
  const collect=`(()=>{const out=[];for(const el of document.querySelectorAll('a[href],button,input:not([type=hidden]),select')) {
    if(el.disabled)continue;const r=el.getBoundingClientRect(),s=getComputedStyle(el);
    if(r.width<8||r.height<8||r.left<0||r.top<0||r.right>innerWidth||r.bottom>innerHeight||s.visibility!=='visible'||+s.opacity<.2)continue;
    const t=document.elementFromPoint((r.left+r.right)/2,(r.top+r.bottom)/2);if(!t||!(t===el||el.contains(t)))continue;
    const label=(el.getAttribute('aria-label')||el.getAttribute('title')||el.getAttribute('placeholder')||el.labels?.[0]?.textContent||el.innerText||el.name||'').trim().replace(/\\s+/g,' ').slice(0,100);
    if(!label)continue;let target='the "'+label+'" '+(el.tagName==='A'?'link':el.tagName==='INPUT'?(el.type==='checkbox'?'checkbox':'field'):el.tagName==='SELECT'?'dropdown':'button');
    const title=el.closest('article,section')?.querySelector('h2')?.textContent?.trim();if(title&&!label.includes(title))target+=' for '+title;
    out.push({target,box:[r.left,r.top,r.right,r.bottom],label,tag:el.tagName,center_hit_test:true});}
    const counts={};for(const e of out)counts[e.target]=(counts[e.target]||0)+1;return out.filter(e=>counts[e.target]===1);})()`;
  async function click(selector){
    const p=await js(`(()=>{const e=document.querySelector(${JSON.stringify(selector)});if(!e||e.disabled)throw Error('not actionable');e.scrollIntoView({block:'center'});const r=e.getBoundingClientRect();return {x:(r.left+r.right)/2,y:(r.top+r.bottom)/2};})()`);
    await cdp('Input.dispatchMouseEvent',{type:'mousePressed',button:'left',clickCount:1,...p});
    await cdp('Input.dispatchMouseEvent',{type:'mouseReleased',button:'left',clickCount:1,...p});
  }
  async function fill(selector,value){
    await click(selector);
    await js(`(()=>{const e=document.querySelector(${JSON.stringify(selector)});Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(e,${JSON.stringify(String(value))});e.dispatchEvent(new Event('input',{bubbles:true}));e.dispatchEvent(new Event('change',{bubbles:true}));})()`);
  }
  for(let index=0;index<scenarios.length;index++) {
    const s=scenarios[index],width=widths[index%3],height=heights[index%3];
    let log='';
    fixture=spawn(process.env.MAGE_PYTHON||'python3',['-u','-I',resolve('scripts/bench/mage-hands/train/shop_fixture.py'),
      '--output',join(output,`fixture-${index}`)],{stdio:['ignore','pipe','pipe']});
    fixture.stdout.on('data',b=>log+=b);fixture.stderr.on('data',b=>log+=b);
    origin='';
    for(let i=0;i<100;i++){const m=log.match(/Fake shop:\s*(http:\/\/127\.0\.0\.1:\d+)\//);if(m){origin=m[1];break;}
      if(fixture.exitCode!==null)throw Error(log);await sleep(50);}
    if(!origin)throw Error('Local fixture not ready');
    await cdp('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:false});
    const navigation=await cdp('Page.navigate',{url:origin+'/catalog'});
    if(navigation.errorText)throw Error('Local navigation failed: '+navigation.errorText);
    await settle();
    async function capture(state){
      const location=await js('({origin:location.origin,title:document.title})');
      if(location.origin!==origin||location.title!=='Fake order sandbox')throw Error('Not the local fake-shop page: '+JSON.stringify(location));
      const elements=await js(collect);if(elements.length<2)throw Error('Too few visible targets');
      const shot=`shots/${hash(s.group+':'+state).slice(0,24)}.png`;
      const image=Buffer.from((await cdp('Page.captureScreenshot',{format:'png'})).data,'base64');writeFileSync(join(output,shot),image);
      pages.push({shot,sha256:hash(image),url:await js('location.href'),family:'shopping-transition',group:s.group,state,
        split:splitFor(s.group),width,height,elements,collection:'scripted-real-transition-not-agent-success'});
      return shot;
    }
    await capture('catalog');
    async function step(label,fn){const before=pages.at(-1).shot;await fn();await settle();const after=await capture(label);
      transitions.push({group:s.group,label,before,after,scripted:true});}
    const selected=`form:has(input[value="${s.item}"])`;
    await step('add-selected',()=>click(`${selected} button`));
    await step('add-distractor',()=>click('form:has(input[value="blue-m-shirt"]) button'));
    await step('cart',()=>click('a[href="/cart"]'));
    await step('remove-distractor',()=>click('form[action="/remove"]:has(input[value="blue-m-shirt"]) button'));
    await step('quantity',async()=>{await fill(`${selected} input[name="quantity"]`,s.qty);await click(`${selected}[action="/quantity"] button`);});
    await step('checkout',()=>click('a[href="/checkout"]'));
    const initial=s.shipping==='express'?'standard':'express';
    async function shipping(value){await js(`(()=>{const e=document.querySelector('select[name="shipping"]');e.value=${JSON.stringify(value)};e.dispatchEvent(new Event('change',{bubbles:true}));})()`);}
    await step('filled-shipping',async()=>{await fill('input[name="name"]','Dataset Shopper '+index);await fill('input[name="street"]',(index+20)+' Fiction Lane');await fill('input[name="postal"]','00000');await shipping(initial);});
    await step('review-before-edit',()=>click('form[action="/review"] button'));
    await step('edit-shipping',()=>click('a[href="/checkout"]'));
    await step('review-final',async()=>{await shipping(s.shipping);await click('form[action="/review"] button');});
    await step('accept-terms',()=>click('input[name="terms"]'));
    await step('complete',()=>click('form[action="/submit"] button'));
    const state=await(await fetch(origin+'/oracle')).json(),orders=Object.values(state.orders);
    const cents={'blue-l-shirt':1800,'silver-mug':900,'cedar-bag':3200};
    if(state.view!=='complete'||orders.length!==1||orders[0].cart[s.item]!==s.qty||Object.keys(orders[0].cart).length!==1||
      orders[0].shipping!==s.shipping||orders[0].total_cents!==cents[s.item]*s.qty+(s.shipping==='express'?800:0))throw Error('Fake-order oracle mismatch');
    oracles.push({group:s.group,expected:s,state,pass:true});
    console.log(JSON.stringify({group:s.group,width,height,pages:pages.filter(p=>p.group===s.group).length,oracle:'pass'}));
    await stop(fixture);fixture=undefined;
  }
  allDone=true;
} catch(e) {failures.push({error:e.message});throw e;
} finally {
  writeFileSync(join(output,'pages.json'),JSON.stringify(pages,null,2)+'\n');
  writeFileSync(join(output,'transitions.json'),JSON.stringify(transitions,null,2)+'\n');
  writeFileSync(join(output,'oracles.json'),JSON.stringify(oracles,null,2)+'\n');
  writeFileSync(join(output,'failures.json'),JSON.stringify(failures,null,2)+'\n');
  if(ws)ws.close();await stop(fixture);await stop(chrome);
  writeFileSync(join(output,'receipt.json'),JSON.stringify({complete:allDone,pages:pages.length,transitions:transitions.length,
    scenarios:oracles.length,failures:failures.length,scope:'local-fake-shop-only',external_requests_blocked:true,
    chrome_stopped:stopped(chrome),fixture_stopped:stopped(fixture)},null,2)+'\n');
}
