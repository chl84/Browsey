/* global window, document, MutationObserver, performance */
import assert from 'node:assert/strict'
export async function startThumbnailObservation(ui) {
  await ui.browser.execute(()=>{
    if(window.__browseyThumbnailObservation) throw Error('Thumbnail observer already exists')
    const samples=[];let lastSignature
    const capture=()=>{
      const cards=[...document.querySelectorAll('.grid [data-path]')].filter(n=>n.getClientRects().length)
      const state={order:cards.map(n=>n.dataset.path),images:cards.map(n=>{
        const img=n.querySelector('img.icon'),source=decodeURIComponent(img?.src??'')
        return {path:n.dataset.path,thumbnail:source.includes('/browsey/thumbs/'),decoded:!!img?.complete&&img.naturalWidth>0,width:img?.naturalWidth??0,height:img?.naturalHeight??0}
      })}
      const signature=JSON.stringify(state)
      if(samples.length<256&&lastSignature!==signature) {samples.push({...state,time:performance.now()});lastSignature=signature}
    }
    const observer=new MutationObserver(capture);observer.observe(document.body,{subtree:true,childList:true,attributes:true})
    document.addEventListener('load',capture,true);window.__browseyThumbnailObservation={samples,observer,capture};capture()
  })
}
export async function finishThumbnailObservation(ui) {
  return ui.browser.execute(()=>{
    const state=window.__browseyThumbnailObservation;if(!state) throw Error('Missing thumbnail observer')
    state.capture();state.observer.disconnect();document.removeEventListener('load',state.capture,true)
    delete window.__browseyThumbnailObservation;return state.samples
  })
}
export function assertThumbnailSnapshots(samples,expected,images) {
  assert.ok(samples.length>0&&samples.length<=256)
  const complete=samples.filter(s=>s.order.length===expected.length)
  assert.ok(complete.length>0,'The complete generated grid must be observed')
  for(const state of complete) assert.deepEqual(state.order,expected,'Late thumbnail completion must preserve the selected sort order')
  for(const path of images) {
    const image=samples.at(-1).images.find(i=>i.path===path)
    assert.ok(image?.thumbnail&&image.decoded&&image.width>0&&image.height>0,'Expected a decoded generated cache thumbnail, not a fallback icon')
  }
}
export async function waitThumbnails(ui,paths) {
  await ui.browser.waitUntil(async()=>ui.browser.execute(paths=>paths.every(path=>{
    const card=[...document.querySelectorAll('.grid [data-path]')].find(n=>n.dataset.path===path),img=card?.querySelector('img.icon')
    return img?.complete&&img.naturalWidth>0&&decodeURIComponent(img.src).includes('/browsey/thumbs/')
  }),paths),{timeout:180_000,interval:100,timeoutMsg:'Generated MTP thumbnails did not decode in the owned grid'})
}
