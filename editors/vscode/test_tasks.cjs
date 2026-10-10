'use strict';

// Source-only template and ECMAScript regex checks, not a VS Code engine emulator.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const config = JSON.parse(fs.readFileSync(path.join(__dirname, 'tasks.json'), 'utf8'));
assert.deepEqual(Object.keys(config).sort(), ['tasks', 'version']);
assert.equal(config.version, '2.0.0');
assert.equal(config.tasks.length, 2);
const [check, format] = config.tasks;
const commonKeys = ['args', 'command', 'label', 'options', 'presentation', 'problemMatcher', 'runOptions', 'type'];
for (const task of config.tasks) {
  assert.deepEqual(Object.keys(task).sort(), commonKeys);
  assert.equal(task.type, 'process');
  assert.equal(task.command, 'oxid');
  assert.deepEqual(task.options, {cwd: '${workspaceFolder}'});
  assert.deepEqual(task.runOptions, {runOn: 'default'});
  assert.deepEqual(task.presentation, {reveal: 'always', panel: 'dedicated'});
  assert.ok(task.label.startsWith('Oxid: '));
}
assert.notEqual(check.label, format.label);
assert.deepEqual(check.args, ['check', '--edition=typed-preview', '--message-format=text', '--', '${file}']);
assert.deepEqual(format.args, ['fmt', '--edition=typed-preview', '--check', '--', '${file}']);
assert.deepEqual(format.problemMatcher, []);

const matcher = check.problemMatcher;
assert.deepEqual(Object.keys(matcher).sort(), ['fileLocation', 'owner', 'pattern', 'severity', 'source']);
assert.equal(matcher.owner, 'oxid-typed-check');
assert.equal(matcher.source, 'Oxid typed-preview');
assert.equal(matcher.severity, 'error');
assert.deepEqual(matcher.fileLocation, ['autodetect', '${workspaceFolder}']);
assert.equal(matcher.pattern.length, 2);
const [header, location] = matcher.pattern;
assert.deepEqual(Object.keys(header).sort(), ['code', 'message', 'regexp']);
assert.deepEqual(Object.keys(location).sort(), ['file', 'line', 'regexp']);
assert.equal(header.code, 1);
assert.equal(header.message, 2);
assert.equal(location.file, 1);
assert.equal(location.line, 2);
const first = new RegExp(header.regexp);
const second = new RegExp(location.regexp);

// Exercise consecutive pairs using the exact configured patterns and captures.
function pairs(text) {
  const lines = text.split('\n');
  const result = [];
  for (let i = 0; i + 1 < lines.length; i++) {
    const a = first.exec(lines[i]);
    const b = second.exec(lines[i + 1]);
    if (a && b) {
      result.push({code: a[header.code], message: a[header.message],
        file: b[location.file], line: Number(b[location.line])});
      i++;
    }
  }
  return result;
}

const paths = [
  '/tmp/my project/entry.ox',
  'C:\\Users\\Example User\\entry.ox',
  '\\\\server\\share name\\entry.ox',
  '/tmp/a:12:34/entry:56.ox',
  'src/child.ox',
  '/tmp/--edition=legacy-0.9.ox',
  '/tmp/a;$(touch forbidden)&b.ox',
  '/tmp/日本語 😀.ox',
];
for (const file of paths) {
  for (const newline of ['\n', '\r\n']) {
    const text = `error[E0200] (type): expected i32${newline}  --> ${file}:12:34${newline}`;
    assert.deepEqual(pairs(text), [{code: 'E0200', message: '(type): expected i32', file, line: 12}]);
  }
  for (const task of config.tasks) {
    const argv = task.args.map(arg => arg === '${file}' ? file : arg);
    assert.equal(argv.length, 5);
    assert.deepEqual(argv.slice(-2), ['--', file]);
  }
}

assert.deepEqual(pairs('error[E0001] (cli): missing file\n'), []);
assert.deepEqual(pairs('typed-preview check ok\n'), []);
assert.deepEqual(pairs('error[E0200] (type): message\n  ::: /tmp/a.ox:1:1: label\n'), []);
assert.deepEqual(pairs('error[E0200] (type): message\n  = note: text\n  --> /tmp/a.ox:1:1\n'), []);
assert.deepEqual(pairs('error[E0200] (type): message\n  --> /tmp/a.ox:0:1\n'), []);
assert.deepEqual(pairs('error[E0200] (type): message\n  --> /tmp/a.ox:1:0\n'), []);
assert.deepEqual(pairs('prefix error[E0200] (type): message\n  --> /tmp/a.ox:1:1\n'), []);
const mixed = [
  'error[E0001] (cli): no source location',
  'error[E0200] (type): first 😀', '  --> /tmp/a.ox:3:8',
  '  ::: /tmp/a.ox:1:1: declaration', '  = note: more detail',
  'error[E0201] (type): second', '  --> /tmp/b.ox:4:2',
  'error[E0202] (type): third', '  --> /tmp/a.ox:5:9',
].join('\n');
assert.deepEqual(pairs(mixed), [
  {code: 'E0200', message: '(type): first 😀', file: '/tmp/a.ox', line: 3},
  {code: 'E0201', message: '(type): second', file: '/tmp/b.ox', line: 4},
  {code: 'E0202', message: '(type): third', file: '/tmp/a.ox', line: 5},
]);
console.log('PASS: task configuration, argv boundaries, and ECMAScript matcher fixtures');
