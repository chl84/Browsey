import assert from 'node:assert/strict'
import { once } from 'node:events'
import { createServer, connect } from 'node:net'
import test from 'node:test'
import { createFaultProxy } from './cloud-fault-proxy.mjs'

test('scoped fault closes an active tunnel and refuses reconnection', { timeout: 5000 }, async () => {
  const target = createServer(socket => socket.on('data', data => socket.write(data)))
  target.listen(0, '127.0.0.1')
  await once(target, 'listening')
  const proxy = await createFaultProxy({ allowed: (host, port) => host === '127.0.0.1' && port === target.address().port })
  let client, retry
  try {
    client = connect(proxy.port, '127.0.0.1')
    client.write(`CONNECT 127.0.0.1:${target.address().port} HTTP/1.1\r\nHost: fixture\r\n\r\n`)
    assert.match(String((await once(client, 'data'))[0]), /200 Connection Established/)
    client.write('owned test bytes')
    assert.equal(String((await once(client, 'data'))[0]), 'owned test bytes')
    const closed = once(client, 'close')
    proxy.dropConnections()
    await closed
    retry = connect(proxy.port, '127.0.0.1')
    retry.write(`CONNECT 127.0.0.1:${target.address().port} HTTP/1.1\r\n\r\n`)
    assert.match(String((await once(retry, 'data'))[0]), /503 Rejected/)
  } finally {
    client?.destroy(); retry?.destroy()
    await proxy.close()
    await new Promise(resolve => target.close(resolve))
  }
})

test('default tunnel rejects non-Microsoft destinations without connecting', { timeout: 5000 }, async () => {
  const proxy = await createFaultProxy()
  const client = connect(proxy.port, '127.0.0.1')
  try {
    client.write('CONNECT localhost:443 HTTP/1.1\r\n\r\n')
    assert.match(String((await once(client, 'data'))[0]), /403 Rejected/)
  } finally { client.destroy(); await proxy.close() }
})
