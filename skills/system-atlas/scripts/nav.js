
const nav=document.querySelector('nav');
const toc=document.querySelector('.toc-root');
const statusText=document.querySelector('.toc-status strong');
const locationSection=document.querySelector('.location-section');
const locationCurrent=document.querySelector('.location-current');
const mobileToggle=document.querySelector('.toc-mobile-toggle');
const groups=[...toc.children].filter(item=>item.querySelector(':scope > ol'));
let activeLink=null;

new ResizeObserver(()=>{
  const height=matchMedia('(max-width:1000px)').matches?nav.getBoundingClientRect().height:0;
  document.documentElement.style.setProperty('--mobile-nav-height',`${height}px`);
}).observe(nav);

function setGroupOpen(group,open){
  group.classList.toggle('is-open',open);
  const button=group.querySelector(':scope > .toc-toggle');
  if(button)button.setAttribute('aria-expanded',String(open));
}

for(const group of groups){
  const link=group.querySelector(':scope > a');
  const button=document.createElement('button');
  button.className='toc-toggle';
  button.type='button';
  button.setAttribute('aria-label',`Show diagrams in ${link.textContent}`);
  button.setAttribute('aria-expanded','false');
  button.addEventListener('click',()=>{
    const open=!group.classList.contains('is-open');
    for(const other of groups)setGroupOpen(other,other===group&&open);
  });
  group.insertBefore(button,group.querySelector(':scope > ol'));
  link.addEventListener('click',()=>setGroupOpen(group,true));
}

function showLocation(target){
  if(!target?.id)return;
  const link=nav.querySelector(`a[href="#${CSS.escape(target.id)}"]`);
  if(!link||link===activeLink)return;
  activeLink?.classList.remove('is-active');
  activeLink=link;
  link.classList.add('is-active');
  link.setAttribute('aria-current','location');
  for(const other of nav.querySelectorAll('[aria-current="location"]')){
    if(other!==link)other.removeAttribute('aria-current');
  }
  statusText.textContent=link.textContent;
  const group=groups.find(item=>item.contains(link));
  const groupLink=group?.querySelector(':scope > a');
  locationSection.textContent=groupLink?.textContent||'ENET System Atlas';
  locationCurrent.textContent=link===groupLink?'Overview':link.textContent;
  if(group)for(const other of groups)setGroupOpen(other,other===group);
  const linkRect=link.getBoundingClientRect();
  const tocRect=toc.getBoundingClientRect();
  if(linkRect.top<tocRect.top||linkRect.bottom>tocRect.bottom){
    toc.scrollTop+=linkRect.top-tocRect.top-toc.clientHeight/3;
  }
  placeConductor(link);
}

mobileToggle.addEventListener('click',()=>{
  const open=nav.classList.toggle('toc-open');
  mobileToggle.setAttribute('aria-expanded',String(open));
  mobileToggle.textContent=open?'Close contents':'Browse contents';
  if(open)scrollTo(0,0);
});

toc.addEventListener('click',event=>{
  if(event.target.closest('a')&&matchMedia('(max-width:1000px)').matches){
    nav.classList.remove('toc-open');
    mobileToggle.setAttribute('aria-expanded','false');
    mobileToggle.textContent='Browse contents';
  }
});

const zoomLevels=[1,1.25,1.5,2];
for(const wrap of document.querySelectorAll('.svgwrap')){
  const frame=document.createElement('div');
  frame.className='diagram-frame';
  wrap.before(frame);
  frame.append(wrap);
  const toolbar=document.createElement('div');
  toolbar.className='diagram-toolbar';
  toolbar.setAttribute('role','group');
  toolbar.setAttribute('aria-label','Diagram view controls');
  toolbar.innerHTML='<button type="button" data-diagram-action="out" aria-label="Zoom out" disabled><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 8h8"/></svg></button><output aria-label="Zoom level" aria-live="polite">100%</output><button type="button" data-diagram-action="in" aria-label="Zoom in"><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 8h8M8 4v8"/></svg></button><button type="button" data-diagram-action="reset" aria-label="Reset zoom" disabled><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M12.5 5.5A5 5 0 1 0 13 9M12.5 5.5v-3M12.5 5.5h-3"/></svg></button><span class="diagram-divider" aria-hidden="true"></span><button type="button" data-diagram-action="maximize" aria-label="Maximize diagram"><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M6 3H3v3M10 3h3v3M6 13H3v-3M10 13h3v-3"/></svg></button>';
  const controls=document.createElement('div');controls.className='figure-controls';
  const h3=frame.closest('figure')?.querySelector('h3');
  if(h3){h3.after(controls);controls.append(toolbar);}else{frame.prepend(toolbar);}
  const maximize=toolbar.querySelector('[data-diagram-action="maximize"]');
  maximize.hidden=!document.fullscreenEnabled;
  toolbar.querySelector('.diagram-divider').hidden=!document.fullscreenEnabled;
}

document.addEventListener('click',event=>{
  const button=event.target.closest('[data-diagram-action]');
  if(!button)return;
  const figure=button.closest('figure');
  if(!figure)return;
  if(button.dataset.diagramAction==='maximize'){
    if(document.fullscreenElement===figure)void document.exitFullscreen();
    else void figure.requestFullscreen();
    return;
  }
  const wrap=figure.querySelector('.svgwrap');
  const toolbar=button.closest('.diagram-toolbar');
  const current=Number(figure.dataset.zoomIndex||0);
  const action=button.dataset.diagramAction;
  const next=action==='reset'?0:Math.max(0,Math.min(zoomLevels.length-1,current+(action==='in'?1:-1)));
  const center=(wrap.scrollLeft+wrap.clientWidth/2)/wrap.scrollWidth;
  const zoom=zoomLevels[next];
  figure.dataset.zoomIndex=String(next);
  wrap.style.setProperty('--zoom-width',`${zoom*100}%`);
  wrap.style.setProperty('--zoom-min-width',`${zoom*720}px`);
  wrap.scrollLeft=center*wrap.scrollWidth-wrap.clientWidth/2;
  toolbar.querySelector('output').value=`${zoom*100}%`;
  toolbar.querySelector('[data-diagram-action="out"]').disabled=next===0;
  toolbar.querySelector('[data-diagram-action="in"]').disabled=next===zoomLevels.length-1;
  toolbar.querySelector('[data-diagram-action="reset"]').disabled=next===0;
});

document.addEventListener('fullscreenchange',()=>{
  for(const button of document.querySelectorAll('[data-diagram-action="maximize"]')){
    const active=document.fullscreenElement===button.closest('figure');
    button.setAttribute('aria-label',active?'Exit full screen':'Maximize diagram');
    button.querySelector('path').setAttribute('d',active?'M6 6H3V3M10 6h3V3M6 10H3v3M10 10h3v3':'M6 3H3v3M10 3h3v3M6 13H3v-3M10 13h3v-3');
  }
});

const locations=[...document.querySelectorAll('main section[id],main figure[id]')];
let ticking=false;
function updateLocation(){
  const marker=innerHeight*.28;
  let current=locations[0];
  for(const location of locations){
    if(location.getBoundingClientRect().top>marker)break;
    current=location;
  }
  showLocation(current);
  ticking=false;
}
addEventListener('scroll',()=>{
  if(!ticking){ticking=true;requestAnimationFrame(updateLocation)}
},{passive:true});
function updateFromHash(){
  const target=document.getElementById(decodeURIComponent(location.hash.slice(1)));
  if(target)showLocation(target);else updateLocation();
}
addEventListener('hashchange',updateFromHash);
addEventListener('load',updateFromHash);
updateFromHash();

// theme toggle: system -> light -> dark
const root=document.documentElement; const order=['system','light','dark'];
const themeBtn=document.createElement('button'); themeBtn.type='button'; themeBtn.className='theme-toggle';
let theme='system'; try{theme=localStorage.getItem('atlas-theme')||(root.dataset.theme||'system')}catch(e){}
function applyTheme(t){theme=t; if(t==='system')root.removeAttribute('data-theme'); else root.dataset.theme=t;
  themeBtn.textContent='Theme: '+t; themeBtn.setAttribute('aria-label','Theme, currently '+t+', click to change'); try{localStorage.setItem('atlas-theme',t)}catch(e){}}
themeBtn.addEventListener('click',()=>applyTheme(order[(order.indexOf(theme)+1)%order.length]));
document.querySelector('.nav-head .sub')?.after(themeBtn); applyTheme(theme);
// progress marker
const figs=[...document.querySelectorAll('main figure[data-index]')];
const total=figs.length; const sections=[...document.querySelectorAll('main section[id]:not(#glossary)')];
const progress=document.createElement('div'); progress.className='toc-progress'; progress.style.cssText='margin-top:.4rem;font-family:var(--mono);font-size:.6rem;letter-spacing:.1em;color:var(--muted)';
document.querySelector('.toc-status')?.appendChild(progress);
new MutationObserver(()=>{
  const a=document.querySelector('nav a.is-active'); if(!a)return;
  const id=a.getAttribute('href').slice(1); const el=document.getElementById(id);
  const fig=el?.closest('figure')||el?.querySelector('figure');
  const sec=el?.closest('section'); const si=sections.indexOf(sec)+1;
  progress.textContent=sec&&sec.id==='glossary'?'Glossary':(fig?`Figure ${fig.dataset.index} of ${total} · `:'')+(si?`Section ${si} of ${sections.length}`:'');
}).observe(nav,{subtree:true,attributes:true,attributeFilter:['class']});
// open a collapsed <details> when its figure is targeted
function openFor(id){const el=document.getElementById(id);const d=el?.closest('details');if(d)d.open=true;}
addEventListener('hashchange',()=>openFor(decodeURIComponent(location.hash.slice(1))));
openFor(decodeURIComponent(location.hash.slice(1)));
nav.addEventListener('click',e=>{const a=e.target.closest('a[href^="#"]');if(a)openFor(a.getAttribute('href').slice(1));});
// search
const search=document.createElement('input'); search.type='search'; search.className='toc-search'; search.placeholder='Search figures and captions';
search.setAttribute('aria-label','Search figures'); document.querySelector('.nav-head .sub')?.after(search);
const items=[...toc.querySelectorAll('li[data-search]')];
const tocEmpty=document.createElement('p');
tocEmpty.className='toc-empty';
tocEmpty.textContent='No figures match that search.';
search.after(tocEmpty);
search.addEventListener('input',()=>{
  const q=search.value.trim().toLowerCase(); nav.classList.toggle('is-searching',!!q);
  for(const li of items)li.classList.toggle('is-hidden',!!q&&!li.dataset.search.includes(q));
  for(const g of groups){const any=!q||[...g.querySelectorAll('li[data-search]')].some(li=>!li.classList.contains('is-hidden'));g.classList.toggle('is-hidden',!any);}
  for(const li of toc.querySelectorAll(':scope > li')){
    if(li.querySelector(':scope > ol'))continue;
    li.classList.toggle('is-hidden',!!q);
  }
  const hits=!!q&&items.some(li=>!li.classList.contains('is-hidden'));
  tocEmpty.classList.toggle('is-shown',!!q&&!hits);
  if(q)placeConductor(null); else if(activeLink)placeConductor(activeLink);
});

var atlasConductor=document.createElement('div');
atlasConductor.className='toc-conductor';
atlasConductor.setAttribute('aria-hidden','true');
toc.prepend(atlasConductor);
if(activeLink)placeConductor(activeLink);
function placeConductor(link){
  if(!atlasConductor)return;
  if(!link||!toc.contains(link)||nav.classList.contains('is-searching')){atlasConductor.classList.remove('is-on');return;}
  const reduce=window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  if(reduce)atlasConductor.style.transition='none';
  atlasConductor.style.willChange='transform';
  atlasConductor.style.transform='translateY('+link.offsetTop+'px) scaleY('+Math.max(link.offsetHeight,1)+')';
  atlasConductor.classList.add('is-on');
  clearTimeout(placeConductor._wc);
  placeConductor._wc=setTimeout(()=>{atlasConductor.style.willChange='auto'},320);
}
let energyTimer=0;
function energize(node){
  const fig=node&&(node.matches&&node.matches('figure')?node:node.closest&&node.closest('figure')||node.querySelector&&node.querySelector('figure'));
  if(!fig)return;
  fig.classList.remove('is-energized');
  void fig.offsetWidth;
  fig.classList.add('is-energized');
  clearTimeout(energyTimer);
  energyTimer=setTimeout(()=>fig.classList.remove('is-energized'),720);
}
toc.addEventListener('click',event=>{
  const a=event.target.closest('a[href^="#"]');
  if(!a)return;
  const dest=document.getElementById(decodeURIComponent(a.getAttribute('href').slice(1)));
  if(dest)requestAnimationFrame(()=>energize(dest));
});
addEventListener('hashchange',()=>{
  const dest=document.getElementById(decodeURIComponent(location.hash.slice(1)));
  if(dest)energize(dest);
});


(function(){
  const reduce=()=>matchMedia('(prefers-reduced-motion: reduce)').matches;
  let active=null;
  function geometryY(el){
    if(el.tagName==='line'){
      return (Number(el.getAttribute('y1'))+Number(el.getAttribute('y2')))/2;
    }
    try{const b=el.getBBox();return b.y+b.height/2;}catch(e){return 0;}
  }
  function collectSteps(svg){
    const texts=[...svg.querySelectorAll('text')];
    const steps=[];
    for(const el of svg.querySelectorAll('line, path')){
      if(!el.getAttribute('marker-end'))continue;
      if(el.tagName==='line'){
        const x1=Number(el.getAttribute('x1')), x2=Number(el.getAttribute('x2'));
        const y1=Number(el.getAttribute('y1')), y2=Number(el.getAttribute('y2'));
        if(Math.abs(x1-x2)<4 && Math.abs(y1-y2)>48)continue;
      }
      const y=geometryY(el);
      const labels=texts.filter(t=>{
        const ty=Number(t.getAttribute('y'));
        return Number.isFinite(ty)&&Math.abs(ty-y)<24;
      });
      steps.push({el,y,labels});
    }
    steps.sort((a,b)=>a.y-b.y||0);
    return steps;
  }
  function lengthOf(el){
    try{if(typeof el.getTotalLength==='function')return el.getTotalLength();}catch(e){}
    if(el.tagName==='line'){
      const dx=Number(el.getAttribute('x2'))-Number(el.getAttribute('x1'));
      const dy=Number(el.getAttribute('y2'))-Number(el.getAttribute('y1'));
      return Math.hypot(dx,dy);
    }
    return 0;
  }
  function clearVisual(step){
    step.el.getAnimations().forEach(a=>a.cancel());
    step.el.style.opacity='';
    step.el.style.strokeDasharray='';
    step.el.style.strokeDashoffset='';
    for(const t of step.labels)t.style.opacity='';
  }
  function showThrough(steps, n, drawLast){
    for(let i=0;i<steps.length;i++){
      const on=i<n;
      const cur=i===n-1;
      steps[i].el.style.opacity=on?'1':'0.12';
      for(const t of steps[i].labels)t.style.opacity=on?'1':'0.12';
      if(on&&cur&&drawLast&&!reduce()){
        const len=lengthOf(steps[i].el);
        if(len>1){
          steps[i].el.style.strokeDasharray=String(len);
          steps[i].el.style.strokeDashoffset=String(len);
          steps[i].el.getAnimations().forEach(a=>a.cancel());
          steps[i].el.animate(
            [{strokeDashoffset:len},{strokeDashoffset:0}],
            {duration:380,easing:'cubic-bezier(0.16, 1, 0.3, 1)',fill:'forwards'}
          );
        }
      }else if(on){
        steps[i].el.style.strokeDasharray='';
        steps[i].el.style.strokeDashoffset='';
      }
    }
  }
  function mount(fig){
    const svg=fig.querySelector('.svgwrap svg');
    if(!svg)return;
    const steps=collectSteps(svg);
    if(steps.length<2)return;
    const bar=document.createElement('div');
    bar.className='seq-play';
    const btn=document.createElement('button');
    btn.type='button';
    btn.className='seq-play-toggle';
    btn.textContent='Play';
    btn.setAttribute('aria-pressed','false');
    btn.setAttribute('aria-label','Play this sequence');
    const scrub=document.createElement('input');
    scrub.type='range';
    scrub.className='seq-play-scrub';
    scrub.min='0';
    scrub.max=String(steps.length);
    scrub.value=String(steps.length);
    scrub.setAttribute('aria-label','Sequence step');
    const count=document.createElement('span');
    count.className='seq-play-count';
    const title=fig.querySelector('h3');
    if(!title)return;
    bar.append(btn,scrub,count);
    const controls=fig.querySelector('.figure-controls');
    if(controls)controls.prepend(bar);else title.after(bar);
    const player={timer:0,n:steps.length,playing:false};
    function setCount(n){
      count.textContent=n+' / '+steps.length;
      scrub.value=String(n);
    }
    function idle(){
      player.playing=false;
      player.n=steps.length;
      btn.textContent='Play';
      btn.setAttribute('aria-pressed','false');
      btn.setAttribute('aria-label','Play this sequence');
      for(const s of steps)clearVisual(s);
      setCount(steps.length);
      if(active===player)active=null;
    }
    function stopTimer(){clearTimeout(player.timer);player.timer=0;}
    function pause(){
      stopTimer();
      player.playing=false;
      btn.textContent=player.n>=steps.length?'Play':'Resume';
      btn.setAttribute('aria-pressed','false');
      btn.setAttribute('aria-label',player.n>=steps.length?'Play this sequence':'Resume this sequence');
      if(player.n>=steps.length)idle();
    }
    function apply(n, draw){
      player.n=n;
      if(n<=0){
        for(const s of steps){
          s.el.getAnimations().forEach(a=>a.cancel());
          s.el.style.opacity='0.12';
          s.el.style.strokeDasharray='';
          s.el.style.strokeDashoffset='';
          for(const t of s.labels)t.style.opacity='0.12';
        }
        setCount(0);
        return;
      }
      showThrough(steps,n,draw);
      setCount(n);
    }
    function tick(){
      if(player.n>=steps.length){pause();return;}
      apply(player.n+1,true);
      player.timer=setTimeout(tick, reduce()?140:520);
    }
    function play(){
      if(active&&active!==player)active.reset();
      active=player;
      if(player.n>=steps.length)apply(0,false);
      player.playing=true;
      btn.textContent='Pause';
      btn.setAttribute('aria-pressed','true');
      btn.setAttribute('aria-label','Pause this sequence');
      tick();
    }
    player.reset=idle;
    btn.addEventListener('click',()=>{ if(player.playing)pause(); else play(); });
    scrub.addEventListener('input',()=>{
      if(active&&active!==player)active.reset();
      active=player;
      stopTimer();
      player.playing=false;
      btn.textContent='Resume';
      btn.setAttribute('aria-pressed','false');
      const n=Number(scrub.value);
      if(n>=steps.length){idle();return;}
      apply(n,false);
      btn.setAttribute('aria-label','Resume this sequence');
    });
    setCount(steps.length);
  }
  for(const fig of document.querySelectorAll('main figure')){
    const eye=fig.querySelector('.eyebrow');
    if(!eye||!/^sequence\b/i.test(eye.textContent.trim()))continue;
    fig.dataset.kind='sequence';
    mount(fig);
  }
})();



// instant glossary tooltips (replaces the browser's delayed title tooltip)
(function(){
  const tip=document.createElement('div');tip.className='tip';tip.setAttribute('role','tooltip');tip.id='atlas-tip';tip.hidden=true;document.body.append(tip);
  let current=null;
  function show(el){
    current=el;tip.innerHTML='';const b=document.createElement('b');b.textContent=el.textContent;tip.append(b,document.createTextNode(el.dataset.tip||''));
    el.setAttribute('aria-describedby','atlas-tip');tip.hidden=false;
    const r=el.getBoundingClientRect();tip.style.left='0px';tip.style.top='0px';tip.classList.add('is-on');
    const w=tip.offsetWidth,h=tip.offsetHeight;
    let x=Math.min(Math.max(8,r.left),innerWidth-w-8);
    let y=r.bottom+8; if(y+h>innerHeight-8)y=r.top-h-8;
    tip.style.left=x+'px';tip.style.top=y+'px';
  }
  function hide(){if(current)current.removeAttribute('aria-describedby');current=null;tip.classList.remove('is-on');tip.hidden=true;}
  document.addEventListener('pointerover',e=>{const el=e.target.closest('abbr.term');if(el&&el!==current)show(el);else if(!el&&current&&!e.target.closest('.tip'))hide();});
  document.addEventListener('pointerout',e=>{if(e.target.closest&&e.target.closest('abbr.term')&&!e.relatedTarget?.closest?.('abbr.term'))hide();});
  document.addEventListener('focusin',e=>{const el=e.target.closest?.('abbr.term');if(el)show(el);});
  document.addEventListener('focusout',e=>{if(e.target.closest?.('abbr.term'))hide();});
  document.addEventListener('click',e=>{const el=e.target.closest('abbr.term');if(el&&matchMedia('(hover: none)').matches){el===current?hide():show(el);}else if(!el)hide();});
  addEventListener('scroll',hide,{passive:true});addEventListener('keydown',e=>{if(e.key==='Escape')hide();});
})();
