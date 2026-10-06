import assert from 'node:assert/strict'
import {test} from 'node:test'
import {assertThumbnailSnapshots} from './mobile-thumbnails.mjs'
test('thumbnail evidence rejects reordered complete grids, fallback icons and undecoded images',()=>{
  const expected=['/owned/a.png','/owned/b.png'],images=expected.map(path=>({path,thumbnail:true,decoded:true,width:128,height:96}))
  const ready={order:expected,images};assertThumbnailSnapshots([{order:[],images:[]},ready],expected,expected)
  assert.throws(()=>assertThumbnailSnapshots([{order:[...expected].reverse(),images},ready],expected,expected),/sort order/)
  for(const defect of [{thumbnail:false},{decoded:false},{width:0}]) assert.throws(()=>assertThumbnailSnapshots([{order:expected,images:images.map(i=>({...i,...defect}))}],expected,expected),/decoded generated/)
})
