#!/usr/bin/env node
// Minimal MCP server to manage repo/project TODO files (Markdown checklists).
// Tools:
// - todo.scan: list TODO_*.md and TODO.md files
// - todo.read: parse tasks from a TODO file
// - todo.update: update task statuses and write back
// - todo.next: propose the next task(s) by simple heuristics (P1 first, then open items)

const fs = require('fs');
const path = require('path');
const rl = require('readline').createInterface({ input: process.stdin });
const JSONRPC = '2.0';

function write(msg) { process.stdout.write(JSON.stringify(msg) + '\n'); }
function isTodoFile(name) { return name === 'TODO.md' || /^TODO_.*\.md$/i.test(name); }
function listTodoFiles(cwd) {
  try { return fs.readdirSync(cwd).filter(isTodoFile); } catch { return []; }
}
function parseTasks(markdown) {
  const lines = markdown.split(/\r?\n/);
  const tasks = [];
  for (const line of lines) {
    const m = line.match(/^\s*- \[( |x|X)\]\s+(.*)$/);
    if (m) {
      const done = m[1].toLowerCase() === 'x';
      const title = m[2].trim();
      let priority = null;
      const pm = title.match(/\[prio:([a-zA-Z0-9]+)\]/);
      if (pm) priority = pm[1];
      tasks.push({ title, done, priority });
    }
  }
  return tasks;
}
function renderTasks(tasks, header) {
  const out = [];
  if (header) out.push(header.trimEnd(), '');
  for (const t of tasks) {
    out.push(`- [${t.done ? 'x' : ' '}] ${t.title}`);
  }
  return out.join('\n') + '\n';
}
function readFileSafe(p) { try { return fs.readFileSync(p, 'utf8'); } catch { return null; } }
function writeFileSafe(p, s) { try { fs.writeFileSync(p, s, 'utf8'); return true; } catch { return false; } }

function initialize(id) {
  return { jsonrpc: JSONRPC, id, result: { protocolVersion: process.env.MCP_SCHEMA_VERSION || '2025-06-18', serverInfo: { name: 'mcp-todo', version: '0.1.0', title: 'TODO Manager' }, capabilities: { tools: { listChanged: false } } } };
}
function listTools(id) {
  return { jsonrpc: JSONRPC, id, result: { tools: [
    { name: 'todo.scan', description: 'List TODO.md and TODO_*.md files in the current directory', inputSchema: { type: 'object', properties: {}, additionalProperties: false } },
    { name: 'todo.read', description: 'Read and parse tasks from a TODO file', inputSchema: { type: 'object', properties: { path: { type: 'string' } }, required: ['path'], additionalProperties: false } },
    { name: 'todo.update', description: 'Update task statuses in a TODO file (by title match)', inputSchema: { type: 'object', properties: { path: { type: 'string' }, updates: { type: 'array', items: { type: 'object', properties: { title: { type: 'string' }, done: { type: 'boolean' } }, required: ['title','done'], additionalProperties: false } }, header: { type: 'string' } }, required: ['path','updates'], additionalProperties: false } },
    { name: 'todo.next', description: 'Propose next tasks from a TODO file', inputSchema: { type: 'object', properties: { path: { type: 'string' }, limit: { type: 'number' } }, required: ['path'], additionalProperties: false } },
  ] } };
}

function call(id, name, params) {
  try {
    if (name === 'todo.scan') {
      return { jsonrpc: JSONRPC, id, result: { files: listTodoFiles(process.cwd()) } };
    }
    if (name === 'todo.read') {
      const p = path.resolve(params.path);
      const s = readFileSafe(p); if (!s) return { jsonrpc: JSONRPC, id, error: { code: -32001, message: 'cannot read file' } };
      return { jsonrpc: JSONRPC, id, result: { tasks: parseTasks(s) } };
    }
    if (name === 'todo.update') {
      const p = path.resolve(params.path);
      const s = readFileSafe(p); if (!s) return { jsonrpc: JSONRPC, id, error: { code: -32001, message: 'cannot read file' } };
      const tasks = parseTasks(s);
      const header = s.split(/\r?\n- \[|\r?\n\s*- \[/)[0] || '';
      const updates = Array.isArray(params.updates) ? params.updates : [];
      for (const u of updates) {
        const idx = tasks.findIndex(t => t.title.replace(/\s+/g,' ').trim() === String(u.title).replace(/\s+/g,' ').trim());
        if (idx >= 0) tasks[idx].done = !!u.done;
      }
      const rendered = renderTasks(tasks, params.header || header);
      const ok = writeFileSafe(p, rendered);
      return ok ? { jsonrpc: JSONRPC, id, result: { ok: true } } : { jsonrpc: JSONRPC, id, error: { code: -32002, message: 'write failed' } };
    }
    if (name === 'todo.next') {
      const p = path.resolve(params.path);
      const s = readFileSafe(p); if (!s) return { jsonrpc: JSONRPC, id, error: { code: -32001, message: 'cannot read file' } };
      const tasks = parseTasks(s).filter(t => !t.done);
      tasks.sort((a,b) => {
        const pa = (a.priority || '').toUpperCase();
        const pb = (b.priority || '').toUpperCase();
        if (pa === pb) return 0; if (pa === 'P1') return -1; if (pb === 'P1') return 1; if (pa === 'P2') return -1; if (pb === 'P2') return 1; return 0;
      });
      const limit = Math.max(1, Math.min(Number(params.limit || 5), 50));
      return { jsonrpc: JSONRPC, id, result: { next: tasks.slice(0, limit) } };
    }
    return { jsonrpc: JSONRPC, id, error: { code: -32601, message: `Unknown tool: ${name}` } };
  } catch (e) { return { jsonrpc: JSONRPC, id, error: { code: -32000, message: String(e && e.message || e) } }; }
}

rl.on('line', line => { let msg; try { msg = JSON.parse(line); } catch { return; } const { id, method, params } = msg || {}; if (method === 'initialize') { write(initialize(id)); return; } if (method === 'tools/list') { write(listTools(id)); return; } if (method === 'tools/call') { write(call(id, params?.name, params?.arguments)); return; } if (id !== undefined) { write({ jsonrpc: JSONRPC, id, error: { code: -32601, message: `Method not implemented: ${method}` } }); } });

