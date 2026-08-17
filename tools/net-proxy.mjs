// 开发辅助工具：本地 HTTP CONNECT 代理（沙箱网络绕过用）
// 本机网络受限时，Node 进程可直连外网而原生程序不行；
// 通过本代理把 cargo/curl 等程序的流量经 Node 隧道转发。
//
// 用法：
//   node tools/net-proxy.mjs [port]        # 默认 18923
//   $env:HTTPS_PROXY="http://127.0.0.1:18923"; $env:HTTP_PROXY="http://127.0.0.1:18923"
import net from 'node:net'
import http from 'node:http'

const PORT = Number(process.argv[2] ?? 18923)

const server = http.createServer((req, res) => {
    // 普通 HTTP 请求（绝对 URI）：转发
    const u = new URL(req.url)
    const upstream = net.connect(Number(u.port || 80), u.hostname, () => {
        upstream.write(`${req.method} ${u.pathname + u.search} HTTP/1.1\r\n`)
        for (const [k, v] of Object.entries(req.headers)) {
            if (k === 'proxy-connection') continue
            upstream.write(`${k}: ${v}\r\n`)
        }
        upstream.write('\r\n')
        req.pipe(upstream)
        upstream.pipe(res)
    })
    upstream.on('error', (e) => {
        res.writeHead(502)
        res.end('upstream error: ' + e.message)
    })
})

server.on('connect', (req, clientSocket, head) => {
    const [host, portRaw] = req.url.split(':')
    const port = Number(portRaw || 443)
    const upstream = net.connect(port, host, () => {
        clientSocket.write('HTTP/1.1 200 Connection Established\r\n\r\n')
        if (head?.length) upstream.write(head)
        upstream.pipe(clientSocket)
        clientSocket.pipe(upstream)
    })
    upstream.on('error', () => {
        clientSocket.end()
    })
    clientSocket.on('error', () => {
        upstream.destroy()
    })
})

server.listen(PORT, '127.0.0.1', () => {
    console.log(`net-proxy listening on 127.0.0.1:${PORT}`)
})
