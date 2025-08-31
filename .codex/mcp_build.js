#!/usr/bin/env node
// Minimal MCP server to detect, build, and test common project types.
// Executes whitelisted commands only, returns stdout/stderr and exit status.

const fs = require('fs');
const cp = require('child_process');
const path = require('path');
const rl = require('readline').createInterface({ input: process.stdin });

const JSONRPC = '2.0';
function write(msg) { process.stdout.write(JSON.stringify(msg) + '\n'); }

function initializeResult(id) {
  return {
    jsonrpc: JSONRPC,
    id,
    result: {
      protocolVersion: process.env.MCP_SCHEMA_VERSION || '2025-06-18',
      serverInfo: { name: 'mcp-build', version: '0.1.0', title: 'Build & Test' },
      capabilities: { tools: { listChanged: false } },
      instructions: 'Use build.detect to choose strategy; use build.run and test.run to execute.'
    }
  };
}

function toolsList(id) {
  return {
    jsonrpc: JSONRPC,
    id,
    result: {
      tools: [
        { name: 'build.detect', description: 'Detects project type', inputSchema: { type: 'object', properties: {}, additionalProperties: false } },
        { name: 'build.run', description: 'Run a build for the project', inputSchema: { type: 'object', properties: { target: { type: 'string', description: 'optional target or script' } }, additionalProperties: false } },
        { name: 'test.run', description: 'Run tests for the project', inputSchema: { type: 'object', properties: { filter: { type: 'string', description: 'test filter if supported' } }, additionalProperties: false } }
      ]
    }
  };
}

function fileExists(p) { try { return fs.existsSync(p); } catch { return false; } }

function detect() {
  const cwd = process.cwd();
  const found = [];
  if (fileExists(path.join(cwd, 'Cargo.toml'))) found.push('rust');
  if (fileExists(path.join(cwd, 'package.json'))) found.push('node');
  if (fileExists(path.join(cwd, 'go.mod'))) found.push('go');
  if (fileExists(path.join(cwd, 'pyproject.toml')) || fileExists(path.join(cwd, 'setup.py'))) found.push('python');
  if (fileExists(path.join(cwd, 'Makefile'))) found.push('make');
  return { types: found };
}

function runCommand(cmd, args, timeoutMs = 10 * 60 * 1000) {
  try {
    const proc = cp.spawnSync(cmd, args, { encoding: 'utf8', maxBuffer: 8 * 1024 * 1024, timeout: timeoutMs });
    const status = (proc.status === null && proc.signal) ? 124 : (proc.status ?? 1);
    return { status, stdout: proc.stdout || '', stderr: proc.stderr || '' };
  } catch (e) {
    return { status: 1, stdout: '', stderr: String(e && e.message || e) };
  }
}

function buildRun(params) {
  const target = (params && params.target || '').trim();
  const d = detect().types;
  if (d.includes('rust')) {
    return runCommand('cargo', target ? ['build', '--', target] : ['build']);
  }
  if (d.includes('node')) {
    // Prefer pnpm > npm if lockfiles exist
    if (fileExists('pnpm-lock.yaml')) return runCommand('pnpm', target ? ['run', target] : ['run', 'build']);
    if (fileExists('package-lock.json')) return runCommand('npm', target ? ['run', target] : ['run', 'build']);
    // fallback to npm
    return runCommand('npm', target ? ['run', target] : ['run', 'build']);
  }
  if (d.includes('go')) {
    return runCommand('go', ['build', './...']);
  }
  if (d.includes('python')) {
    if (fileExists('pyproject.toml')) return runCommand('python3', ['-m', 'build']);
    return runCommand('python3', ['setup.py', 'sdist', 'bdist_wheel']);
  }
  if (d.includes('make')) {
    return runCommand('make', target ? [target] : ['build']);
  }
  return { status: 2, stdout: '', stderr: 'no supported project type detected' };
}

function testRun(params) {
  const filter = (params && params.filter || '').trim();
  const d = detect().types;
  if (d.includes('rust')) {
    return runCommand('cargo', filter ? ['test', filter] : ['test']);
  }
  if (d.includes('node')) {
    if (fileExists('pnpm-lock.yaml')) return runCommand('pnpm', filter ? ['test', '--', filter] : ['test']);
    if (fileExists('package-lock.json')) return runCommand('npm', filter ? ['run', 'test', '--', filter] : ['run', 'test']);
    return runCommand('npm', filter ? ['run', 'test', '--', filter] : ['run', 'test']);
  }
  if (d.includes('go')) {
    return runCommand('go', filter ? ['test', './...', '-run', filter] : ['test', './...']);
  }
  if (d.includes('python')) {
    // best-effort: pytest if available per project config
    return runCommand('pytest', filter ? [filter] : []);
  }
  if (d.includes('make')) {
    return runCommand('make', filter ? ['test', filter] : ['test']);
  }
  return { status: 2, stdout: '', stderr: 'no supported project type detected' };
}

function toolsCall(id, name, params) {
  try {
    if (name === 'build.detect') return { jsonrpc: JSONRPC, id, result: detect() };
    if (name === 'build.run') return { jsonrpc: JSONRPC, id, result: buildRun(params || {}) };
    if (name === 'test.run') return { jsonrpc: JSONRPC, id, result: testRun(params || {}) };
    return { jsonrpc: JSONRPC, id, error: { code: -32601, message: `Unknown tool: ${name}` } };
  } catch (e) {
    return { jsonrpc: JSONRPC, id, error: { code: -32000, message: String(e && e.message || e) } };
  }
}

rl.on('line', line => {
  let msg; try { msg = JSON.parse(line); } catch { return; }
  const { id, method, params } = msg || {};
  if (method === 'initialize') { write(initializeResult(id)); return; }
  if (method === 'tools/list') { write(toolsList(id)); return; }
  if (method === 'tools/call') { write(toolsCall(id, params?.name, params?.arguments)); return; }
  if (id !== undefined) { write({ jsonrpc: JSONRPC, id, error: { code: -32601, message: `Method not implemented: ${method}` } }); }
});

