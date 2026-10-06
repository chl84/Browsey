/* global document */
import assert from 'node:assert/strict'
import {child} from './scope.mjs'
import {recordPart} from './report.mjs'
import {verifyTree} from './editing.mjs'
import {releasedResources} from './resources.mjs'
import {nativeInput,ownedWindow,observeDrag,endDragObservation,elementPoint,dragPoints} from './drag.mjs'
const parts=['list-rows-cancel','list-background-copy','grid-cards-cancel','grid-background-copy']
export const feedbackManifest=plan=>{assert.deepEqual(plan.targets.map(t=>t.kind),['local']);return [{id:'desktop-feedback',name:'Native drag feedback under listing load',providers:['local'],partIds:parts}]}
export async function feedback(plan,fixture,ui,record) {
  await record('Native drag feedback under listing load',async result=>{
    for(const id of parts)await recordPart(result,id,async part=>{
      result.phase='setup';const base=child(plan.targets[0].files,id),dest=child(base,'destination'),source=child(base,'000-source.txt'),expected=new Map([['destination',null],['000-source.txt','source\n']])
      await fixture.mkdir(base);await fixture.mkdir(dest);await fixture.write(source,'source\n')
      for(let i=0;i<120;i++){const name=`row-${String(i).padStart(3,'0')}.txt`;await fixture.write(child(base,name),`${i}\n`);expected.set(name,`${i}\n`)}
      const view=id.startsWith('grid')?'grid':'list';await ui.navigate(base);await ui.setView(view);await ownedWindow(ui,[0,0,980,950]);await ui.select(source)
      const points=await ui.browser.execute(source=>[...document.querySelectorAll('[data-path]')].filter(n=>n.dataset.path!==source&&n.dataset.dropPath==='').map(n=>{const r=n.getBoundingClientRect();return [Math.round(r.left+r.width/2),Math.round(r.top+r.height/2)]}).filter(p=>p[0]>220&&p[0]<970&&p[1]>100&&p[1]<870).slice(0,40),source)
      assert.ok(points.length>=10,'Many actually visible rows/cards required')
      // The right edge of the listing is real empty space, not another card.
      const background=await ui.browser.execute(view=>{const n=document.querySelector(view==='list'?'.rows':'.grid'),r=n.getBoundingClientRect();return [Math.round(r.right-8),Math.round(Math.min(r.bottom-30,880))]},view)
      const a=await elementPoint(ui,source),b=await elementPoint(ui,dest),empty=dragPoints([background[0]-5,background[1]-140],background,14)
      await observeDrag(ui);part.ui='STARTED';result.phase='ui'
      try {
        part.gesture=await nativeInput(ui,'drag',{points:[a,[a[0]+35,a[1]],...points,...empty,...(id.endsWith('copy')?dragPoints(background,b,12):[])],mode:'copy',modifier_after_press:true,cancel:id.endsWith('cancel')})
        if(id.endsWith('copy'))await ui.idle({resultPath:source});else await ui.idle()
      } finally {part.observation=await endDragObservation(ui)}
      const events=part.observation.events,frames=part.observation.frames.filter(f=>f.ghost)
      assert.ok(events.some(e=>e.type==='dragstart'&&e.trusted),'Actual trusted source gesture')
      assert.ok(events.filter(e=>e.type==='dragover'&&e.trusted||e.type==='tauri://drag-over').length>=10,'Real native/event cadence required')
      assert.ok(frames.length>=10,'Real rendered frame samples required')
      assert.equal(new Set(frames.map(f=>f.node)).size,1,'Feedback node must survive rows/cards and background changes')
      assert.ok(frames.some(f=>f.text.includes('Cannot drop here')),'Rejected file hover must be observed')
      assert.ok(frames.some(f=>f.text.includes('Copy')),'Allowed background/folder hover must recover the label')
      const samples=frames.filter(f=>f.point).map(f=>({time:f.time,delta:Math.hypot(f.left-f.point.x-12,f.top-f.point.y-12)}))
      part.measurement={eventCount:events.length,frameCount:frames.length,distinctPositions:new Set(frames.map(f=>`${f.left},${f.top}`)).size,matchedFrames:samples.filter(s=>s.delta===0).length,maxObservedDelta:Math.max(...samples.map(s=>s.delta)),scope:'Observed native pointer/event-to-frame positions; no invented performance threshold'}
      assert.ok(part.measurement.distinctPositions>=10&&part.measurement.matchedFrames>=10,'Feedback must visibly follow real input across many positions')
      await ui.browser.waitUntil(async()=>!await(await ui.browser.$('.ghost')).isExisting(),{timeout:5000,timeoutMsg:'Cannot-drop feedback remained stuck after release'})
      result.phase='verification';if(id.endsWith('copy'))expected.set('destination/000-source.txt','source\n')
      await verifyTree(fixture,base,expected);await releasedResources(ui,plan.runId);part.ui='ACKNOWLEDGED';part.verification='INDEPENDENT_MATCH'
    })
  },{id:'desktop-feedback',providers:['local']})
}
