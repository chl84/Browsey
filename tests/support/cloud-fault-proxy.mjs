// Test-only loopback CONNECT tunnel. TLS bytes are forwarded, never decoded/logged.
import { createServer } from 'node:http'
import { connect } from 'node:net'
import { existsSync, writeFileSync } from 'node:fs'
import { isAbsolute } from 'node:path'
import { fileURLToPath } from 'node:url'

const microsoftTarget = (host, port) => port === 443 && [
  'onedrive.com', '1drv.com', '1drv.ms', 'sharepoint.com',
  'graph.microsoft.com', 'login.microsoftonline.com', 'login.live.com',
  'my.microsoftpersonalcontent.com',
].some(domain => host === domain || host.endsWith(`.${domain}`))

export async function createFaultProxy({ allowed = microsoftTarget } = {}) {
  const sockets = new Set()
  let blocked = false
  const server = createServer((_req, res) => { res.writeHead(405).end() })
  server.on('connect', (request, client, head) => {
    const match = /^([a-zA-Z0-9.-]+):(\d+)$/.exec(request.url ?? '')
    if (blocked || !match || !allowed(match[1].toLowerCase(), Number(match[2]))) {
      client.end(`HTTP/1.1 ${blocked ? 503 : 403} Rejected\r\nConnection: close\r\n\r\n`)
      return
    }
    const upstream = connect(Number(match[2]), match[1])
    for (const socket of [client, upstream]) {
      sockets.add(socket)
      socket.on('close', () => sockets.delete(socket))
      socket.on('error', () => { client.destroy(); upstream.destroy() })
    }
    upstream.once('connect', () => {
      if (blocked) { client.destroy(); upstream.destroy(); return }
      client.write('HTTP/1.1 200 Connection Established\r\n\r\n')
      if (head.length) upstream.write(head)
      client.pipe(upstream)
      upstream.pipe(client)
    })
    client.once('close', () => upstream.destroy())
    upstream.once('close', () => client.destroy())
  })
  await new Promise((resolve, reject) => {
    server.once('error', reject)
    server.listen(0, '127.0.0.1', resolve)
  })
  return {
    port: server.address().port,
    dropConnections() { blocked = true; for (const socket of sockets) socket.destroy() },
    close() {
      blocked = true
      for (const socket of sockets) socket.destroy()
      return new Promise(resolve => server.close(resolve))
    },
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [portFile, faultFile] = process.argv.slice(2)
  if (process.env.BROWSEY_TEST_CLOUD_WRITE_APPROVED !== 'yes'
      || !portFile || !faultFile || !isAbsolute(portFile) || !isAbsolute(faultFile)
      || existsSync(portFile) || existsSync(faultFile)) {
    throw new Error('Explicit opt-in and new absolute fixture marker paths required')
  }
  const proxy = await createFaultProxy()
  writeFileSync(portFile, JSON.stringify({ port: proxy.port }), { flag: 'wx', mode: 0o600 })
  const timer = setInterval(() => { if (existsSync(faultFile)) proxy.dropConnections() }, 25)
  process.once('SIGTERM', async () => { clearInterval(timer); await proxy.close() })
}
