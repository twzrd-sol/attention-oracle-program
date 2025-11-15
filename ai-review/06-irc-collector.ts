#!/usr/bin/env tsx
/*
  Twitch IRC Collector (production-grade, minimal deps)
  - TLS to irc.chat.twitch.tv:6697
  - CAP: tags, commands, membership
  - Events: chat, sub/resub, subgift, bits (if tagged), info
  - Persist raw events and per-minute rollups to Postgres (optional)
  - Rolling NDJSON + snapshot JSON under exports/
*/
import tls from 'node:tls'
import fs from 'node:fs'
import path from 'node:path'
import { Pool } from 'pg'

type EvType = 'chat' | 'sub' | 'subgift' | 'bits' | 'info'

type Event = {
  ts: string
  type: EvType
  channel: string
  user?: string
  targetUser?: string
  count?: number
  bits?: number
  text?: string
  msgId?: string
  ircId?: string
}

const HOST = 'irc.chat.twitch.tv'
const PORT = 6697

function env(name: string, d?: string) {
  const v = process.env[name]
  return (v && v.trim()) || d || ''
}

function randJustinFan() {
  return 'justinfan' + Math.floor(10000 + Math.random() * 89999)
}

function ensureExportsDir() {
  const p = path.resolve('clean-hackathon/exports')
  fs.mkdirSync(p, { recursive: true })
  return p
}

function parseTags(tagStr: string): Record<string, string> {
  const out: Record<string, string> = {}
  if (!tagStr) return out
  for (const kv of tagStr.split(';')) {
    const [k, v = ''] = kv.split('=')
    if (!k) continue
    const cleaned = v.replace(/\\s/g, ' ')
    try { out[k] = decodeURIComponent(cleaned) } catch { out[k] = cleaned }
  }
  return out
}

function usernameFromPrefix(prefix: string): string | null {
  const m = /^:([^!]+)!/.exec(prefix)
  return m ? m[1].toLowerCase() : null
}

function secs() { return Math.floor(Date.now() / 1000) }

async function main() {
  const channels = (env('CHANNELS', '') || '').split(',').map(s => s.trim().toLowerCase()).filter(Boolean)
  if (channels.length === 0) {
    console.error('Set CHANNELS=comma,list (e.g., n3on,xqc)')
    process.exit(2)
  }
  const SNAPSHOT_EVERY_SEC = parseInt(env('SNAPSHOT_EVERY_SEC', '120'), 10) || 120
  const outDir = ensureExportsDir()
  const outBase = channels.join('+')
  const ndjsonPath = path.join(outDir, `${outBase}-events-${new Date().toISOString().slice(0,10)}.ndjson`)
  const snapshotPath = path.join(outDir, `${outBase}-snapshot.json`)

  const nick = env('TWITCH_NICK') || randJustinFan()
  const pass = env('TWITCH_OAUTH') || 'SCHMOOPIIE'

  const dbUrl = env('DATABASE_URL')
  // Prefer local socket when available; disable SSL for socket/TCP-local
  let pool: Pool | null = null
  if (dbUrl) {
    const lower = dbUrl.toLowerCase()
    const isSocket = lower.includes('%2fvar%2frun%2fpostgresql') || lower.includes('/var/run/postgresql')
    pool = new Pool({ connectionString: dbUrl, ssl: isSocket ? false : undefined as any })
  } else {
    const socketHost = env('DATABASE_HOST', '/var/run/postgresql')
    pool = new Pool({ database: 'twzrd', user: 'twzrd', host: socketHost, ssl: false as any })
  }

  if (pool) await initSchema(pool)

  // Stats
  const minuteKey = () => Math.floor(Date.now() / 60000)
  let currentMinute = minuteKey()
  let minuteCounts: Record<string, { msgs: number, gifts: number, subs: number, bits: number, chatters: Set<string> }> = {}
  channels.forEach(c => minuteCounts[c] = { msgs: 0, gifts: 0, subs: 0, bits: 0, chatters: new Set() })

  const socket = tls.connect(PORT, HOST, { servername: HOST })
  socket.setEncoding('utf8')

  function send(line: string) { socket.write(line + '\r\n') }

  socket.once('secureConnect', () => {
    send(`PASS ${pass}`)
    send(`NICK ${nick}`)
    send('CAP REQ :twitch.tv/tags twitch.tv/commands twitch.tv/membership')
    for (const ch of channels) send(`JOIN #${ch}`)
    console.log(`[irc] connected as ${nick}, channels=${channels.join(',')}`)
  })

  let buf = ''
  const dedupe = new Set<string>() // twitch tags id/msg-id

  const writeEvent = (ev: Event) => {
    try { fs.appendFileSync(ndjsonPath, JSON.stringify(ev) + '\n') } catch {}
  }

  const buffer: Event[] = []
  let legacyEventsTable = false
  async function flushBuffer() {
    if (!pool || buffer.length === 0) return
    const toWrite = buffer.splice(0, buffer.length)
    const client = await pool.connect()
    try {
      if (legacyEventsTable) {
        // Insert into existing schema: (message_id, event_type, payload, created_at)
        const txt = `INSERT INTO twitch_events (message_id, event_type, payload, created_at)
          VALUES ${toWrite.map((_, i) => `($${i*4+1}, $${i*4+2}, $${i*4+3}::jsonb, $${i*4+4})`).join(',')}
          ON CONFLICT (message_id) DO NOTHING`
        const vals: any[] = []
        for (const e of toWrite) {
          const id = e.ircId || `${e.type}-${e.channel}-${Date.now()}-${Math.random().toString(36).slice(2)}`
          vals.push(id, e.type, JSON.stringify(e), e.ts)
        }
        await client.query(txt, vals)
      } else {
        const txt = `INSERT INTO twitch_events_raw
          (ts, type, channel, username, target_username, count, bits, text, msg_id, irc_id)
          VALUES ${toWrite.map((_, i) => `($${i*10+1}, $${i*10+2}, $${i*10+3}, $${i*10+4}, $${i*10+5}, $${i*10+6}, $${i*10+7}, $${i*10+8}, $${i*10+9}, $${i*10+10})`).join(',')}
          ON CONFLICT (irc_id) DO NOTHING`
        const vals: any[] = []
        for (const e of toWrite) {
          vals.push(e.ts, e.type, e.channel, e.user || null, e.targetUser || null, e.count || null, e.bits || null, e.text || null, e.msgId || null, e.ircId || null)
        }
        await client.query(txt, vals)
      }
    } catch (e:any) {
      console.error('[db] insert error', e.message)
    } finally { client.release() }
  }

  function rotateMinute() {
    const mk = minuteKey()
    if (mk === currentMinute) return
    // write minute stats
    const stamp = new Date(currentMinute * 60000).toISOString()
    const writes = Object.entries(minuteCounts).map(async ([ch, m]) => {
      const unique = m.chatters.size
      const row = { ts: stamp, channel: ch, messages: m.msgs, gifts: m.gifts, new_subs: m.subs, bits: m.bits, unique_chatters: unique }
      // snapshot refresh
      try { writeSnapshot() } catch {}
      if (!pool) return
      const q = `INSERT INTO twitch_minute_stats
        (minute_ts, channel, messages, gifts, new_subs, bits, unique_chatters)
        VALUES ($1,$2,$3,$4,$5,$6,$7)
        ON CONFLICT (minute_ts, channel) DO UPDATE SET
          messages=EXCLUDED.messages, gifts=EXCLUDED.gifts, new_subs=EXCLUDED.new_subs, bits=EXCLUDED.bits, unique_chatters=EXCLUDED.unique_chatters`
      await pool.query(q, [stamp, ch, m.msgs, m.gifts, m.subs, m.bits, unique])
    })
    Promise.allSettled(writes).then(()=>{}).catch(()=>{})
    // reset
    currentMinute = mk
    minuteCounts = {}
    channels.forEach(c => minuteCounts[c] = { msgs: 0, gifts: 0, subs: 0, bits: 0, chatters: new Set() })
  }

  function push(ev: Event) {
    writeEvent(ev)
    buffer.push(ev)
    const ch = ev.channel
    if (!minuteCounts[ch]) minuteCounts[ch] = { msgs: 0, gifts: 0, subs: 0, bits: 0, chatters: new Set() }
    if (ev.type === 'chat') {
      minuteCounts[ch].msgs++
      if (ev.user) minuteCounts[ch].chatters.add(ev.user)
    } else if (ev.type === 'sub') minuteCounts[ch].subs++
    else if (ev.type === 'subgift') minuteCounts[ch].gifts += ev.count || 1
    else if (ev.type === 'bits') minuteCounts[ch].bits += ev.bits || 0
  }

  function writeSnapshot() {
    // derive top lists from recent minute window (simple: from current minuteCounts)
    const totals = { messages: 0, gifts: 0, new_subs: 0, bits: 0 }
    const chat = new Map<string, number>()
    const gifts = new Map<string, number>()
    const subs = new Map<string, number>()
    try {
      // read last ~100 events from NDJSON to build quick tops
      const data = fs.readFileSync(ndjsonPath, 'utf8').trim().split('\n').slice(-1000)
      for (const line of data) {
        const e: Event = JSON.parse(line)
        if (e.type === 'chat') { totals.messages++; if (e.user) chat.set(e.user, (chat.get(e.user)||0)+1) }
        if (e.type === 'sub') { totals.new_subs++; if (e.user) subs.set(e.user, (subs.get(e.user)||0)+1) }
        if (e.type === 'subgift') { totals.gifts += e.count || 1; if (e.user) gifts.set(e.user, (gifts.get(e.user)||0)+(e.count||1)) }
        if (e.type === 'bits') { totals.bits += e.bits || 0 }
      }
    } catch {}
    const top = (m: Map<string, number>) => Array.from(m.entries()).sort((a,b)=>b[1]-a[1]).slice(0,20)
    const payload = {
      channels,
      started_at: startedAt.toISOString(),
      updated_at: new Date().toISOString(),
      duration_minutes: Math.round((Date.now()-startedAt.getTime())/60000),
      totals,
      top_chatters: top(chat),
      top_gifters: top(gifts),
      top_new_subs: top(subs),
    }
    try { fs.writeFileSync(snapshotPath, JSON.stringify(payload, null, 2)) } catch {}
  }

  const startedAt = new Date()
  let snapshotTimer: NodeJS.Timeout | null = null
  if (SNAPSHOT_EVERY_SEC > 0) snapshotTimer = setInterval(writeSnapshot, SNAPSHOT_EVERY_SEC*1000)

  socket.on('data', (chunk: string) => {
    buf += chunk
    let idx
    while ((idx = buf.indexOf('\r\n')) >= 0) {
      const line = buf.slice(0, idx)
      buf = buf.slice(idx + 2)
      handleLine(line)
    }
  })

  socket.on('error', (e) => console.error('[irc] socket error', e.message))
  socket.on('close', () => console.log('[irc] connection closed'))

  async function handleLine(line: string) {
    if (line.startsWith('PING')) { send('PONG :tmi.twitch.tv'); return }

    let tags: Record<string,string> = {}
    let rest = line
    if (rest.startsWith('@')) {
      const space = rest.indexOf(' ')
      const raw = rest.slice(1, space)
      tags = parseTags(raw)
      rest = rest.slice(space+1)
    }
    let prefix = ''
    if (rest.startsWith(':')) {
      const space = rest.indexOf(' ')
      prefix = rest.slice(0, space)
      rest = rest.slice(space + 1)
    }
    const sp = rest.split(' :')
    const left = sp[0]
    const trailing = sp.length > 1 ? sp.slice(1).join(' :') : ''
    const parts = left.split(' ')
    const command = parts[0]
    const args = parts.slice(1)

    const ch = (args[0]||'').replace('#','').toLowerCase()
    const user = (tags['login'] || usernameFromPrefix(prefix) || '').toLowerCase()
    const ircId = tags['id'] || tags['msg-id'] || undefined
    const evBase: Partial<Event> = { ts: new Date().toISOString(), channel: ch, user, ircId }
    if (ircId && dedupe.has(ircId)) return

    if (command === 'PRIVMSG') {
      if (ircId) dedupe.add(ircId)
      push({ ...(evBase as any), type: 'chat', text: trailing })
      return
    }
    if (command === 'USERNOTICE') {
      const msgId = tags['msg-id'] || ''
      // Sub / resub
      if (msgId === 'sub' || msgId === 'resub' || msgId === 'rewardgift') {
        if (ircId) dedupe.add(ircId)
        push({ ...(evBase as any), type: 'sub', msgId })
        return
      }
      // Mass gift
      if (msgId === 'submysterygift') {
        const n = parseInt(tags['msg-param-mass-gift-count'] || '1', 10) || 1
        if (ircId) dedupe.add(ircId)
        push({ ...(evBase as any), type: 'subgift', count: n, msgId })
        return
      }
      // Bits (cheers sometimes arrive as PRIVMSG with bits tag)
    }
    if (command === 'NOTICE' || command === 'CLEARCHAT' || command === 'CLEARMSG') {
      // informative; ignore for stats
      return
    }
    // Bits via PRIVMSG tag
    const bits = parseInt(tags['bits'] || '0', 10)
    if (bits > 0) {
      if (ircId) dedupe.add(ircId)
      push({ ...(evBase as any), type: 'bits', bits, text: trailing })
    }
  }

  // background jobs
  setInterval(() => { rotateMinute(); flushBuffer().catch(()=>{}) }, 3000)
}

async function initSchema(pool: Pool) {
  const client = await pool.connect()
  try {
    // Detect legacy schema
    const info = await client.query(`SELECT column_name FROM information_schema.columns WHERE table_name='twitch_events'`)
    const cols = info.rows.map((r:any)=>r.column_name)
    legacyEventsTable = cols.includes('message_id') && cols.includes('event_type') && cols.includes('payload')
    if (!legacyEventsTable) {
      await client.query(`CREATE TABLE IF NOT EXISTS twitch_events_raw (
        ts TIMESTAMPTZ NOT NULL,
        type TEXT NOT NULL,
        channel TEXT NOT NULL,
        username TEXT,
        target_username TEXT,
        count INTEGER,
        bits INTEGER,
        text TEXT,
        msg_id TEXT,
        irc_id TEXT UNIQUE,
        PRIMARY KEY (irc_id)
      )`)
    }
    await client.query(`CREATE TABLE IF NOT EXISTS twitch_minute_stats (
      minute_ts TIMESTAMPTZ NOT NULL,
      channel TEXT NOT NULL,
      messages INTEGER DEFAULT 0,
      gifts INTEGER DEFAULT 0,
      new_subs INTEGER DEFAULT 0,
      bits INTEGER DEFAULT 0,
      unique_chatters INTEGER DEFAULT 0,
      PRIMARY KEY (minute_ts, channel)
    )`)
  } finally { client.release() }
}

main().catch((e) => { console.error(e); process.exit(1) })
