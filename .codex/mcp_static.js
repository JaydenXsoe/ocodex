#!/usr/bin/env node
// Minimal MCP server for linting and formatting across common ecosystems.
// Tools: static.detect, lint.code, format.code

const fs = require('fs');
const cp = require('child_process');
const rl = require('readline').createInterface({ input: process.stdin });
const JSONRPC = '2.0';

function write(msg) { process.stdout.write(JSON.stringify(msg) + '\n'); }
function fileExists(p) { try { return fs.existsSync(p); } catch { return false; } }
function run(cmd, args) {
  try {
    const out = cp.spawnSync(cmd, args, { encoding: 'utf8', maxBuffer: 8*1024*1024 });
    return { status: out.status ?? 1, stdout: out.stdout || '', stderr: out.stderr || '' };
  } catch (e) { return { status: 127, stdout: '', stderr: String(e && e.message || e) }; }
}
function detect() {
  const types = [];
  if (fileExists('Cargo.toml')) types.push('rust');
  if (fileExists('package.json')) types.push('node');
  if (fileExists('pyproject.toml') || fileExists('setup.cfg') || fileExists('setup.py')) types.push('python');
  return { types };
}
function lint() {
  const d = detect().types;
  const results = [];
  if (d.includes('rust')) results.push({ tool: 'cargo clippy', ...run('cargo', ['clippy', '--', '-D', 'warnings']) });
  if (d.includes('node')) {
    if (fileExists('node_modules/.bin/eslint')) results.push({ tool: 'eslint', ...run('node', ['node_modules/.bin/eslint', '.', '--max-warnings', '0']) });
    else if (fileExists('package.json')) results.push({ tool: 'npm run lint', ...run('npm', ['run', 'lint']) });
  }
  if (d.includes('python')) {
    if (fileExists('pyproject.toml') || fileExists('setup.cfg')) results.push({ tool: 'flake8', ...run('flake8', ['.']) });
  }
  return { results };
}
function fmt() {
  const d = detect().types;
  const results = [];
  if (d.includes('rust')) results.push({ tool: 'cargo fmt', ...run('cargo', ['fmt']) });
  if (d.includes('node')) {
    if (fileExists('node_modules/.bin/prettier')) results.push({ tool: 'prettier', ...run('node', ['node_modules/.bin/prettier', '--write', '.']) });
    else if (fileExists('package.json')) results.push({ tool: 'npm run format', ...run('npm', ['run', 'format']) });
  }
  if (d.includes('python')) results.push({ tool: 'black', ...run('black', ['.']) });
  return { results };
}

function initialize(id) {
  return { jsonrpc: JSONRPC, id, result: { protocolVersion: process.env.MCP_SCHEMA_VERSION || '2025-06-18', serverInfo: { name: 'mcp-static', version: '0.1.0', title: 'Static Analysis' }, capabilities: { tools: { listChanged: false } } } };
}
function listTools(id) {
  return { jsonrpc: JSONRPC, id, result: { tools: [
    { name: 'static.detect', description: 'Detects project languages for static tooling', inputSchema: { type: 'object', properties: {}, additionalProperties: false } },
    { name: 'lint.code', description: 'Run linters across detected languages', inputSchema: { type: 'object', properties: {}, additionalProperties: false } },
    { name: 'format.code', description: 'Run code formatters across detected languages', inputSchema: { type: 'object', properties: {}, additionalProperties: false } },
  ] } };
}
function call(id, name) {
  try {
    if (name === 'static.detect') return { jsonrpc: JSONRPC, id, result: detect() };
    if (name === 'lint.code') return { jsonrpc: JSONRPC, id, result: lint() };
    if (name === 'format.code') return { jsonrpc: JSONRPC, id, result: fmt() };
    return { jsonrpc: JSONRPC, id, error: { code: -32601, message: `Unknown tool: ${name}` } };
  } catch (e) {
    return { jsonrpc: JSONRPC, id, error: { code: -32000, message: String(e && e.message || e) } };
  }
}

rl.on('line', line => {
  let msg; try { msg = JSON.parse(line); } catch { return; }
  const { id, method, params } = msg || {};
  if (method === 'initialize') { write(initialize(id)); return; }
  if (method === 'tools/list') { write(listTools(id)); return; }
  if (method === 'tools/call') { write(call(id, params?.name)); return; }
  if (id !== undefined) { write({ jsonrpc: JSONRPC, id, error: { code: -32601, message: `Method not implemented: ${method}` } }); }
});

