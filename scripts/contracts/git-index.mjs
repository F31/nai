/// 仓库文件集：**纯静态**读取 `.git/index`（Git 的 DIRC 格式）。
///
/// 为什么不用 `git ls-files`：**不依赖子进程**。子进程能力是**宿主瞬态**——W74 当时实测 `git`（裸名与
/// 绝对路径）、`process.execPath`、`cmd.exe`、`sh` 一律报 `EBUSY`，但那是**一次宿主观测**（换一台机器、
/// 甚至换一次会话都可能不同）⇒ 宿主瞬态 MUST NOT 成为判定输入（铁律二）。读一个文件不受任何这类
/// 约束，也不引入「工具不在 PATH 上」这一类额外的失败面。
/// ⇒ W74 报告里「取仓库文件集必须派生子进程」的陈述不成立，F2 有根除路径。
///
/// 为什么是「**已跟踪**」而不是「git 会发布的内容」：守卫跑在**提交前**，此时「未跟踪」
/// 正是「**尚未承诺**为仓库内容」。`git ls-files --cached --others --exclude-standard`
/// 会把未跟踪但未被忽略的文件收进来 ⇒ 那个伪造的消费者也在集合里，**修不了 F2**。
///
/// 为什么是「路径」而不是「内容」：已跟踪文件的**路径**恒在索引里，与工作区内容是否已被修改
/// 无关 ⇒ 不会误伤「改了但还没 `git add`」的老文件。
import { readFile, stat } from 'node:fs/promises';
import path from 'node:path';

const HEADER_BYTES = 12;
/// v2/v3/v4 的固定头长度：ctime(8)+mtime(8)+dev/ino(8)+mode(4)+uid/gid(8)+size(4)+object(20)+flags(2)
const FIXED_BYTES = 62;
/// flags 的第 14 位。置位时条目后面多 2 字节扩展 flags（v3 起允许，v2 里出现即为损坏）。
const FLAG_EXTENDED = 0x4000;

/// pack 风格的变长整数，v4 用它编码「与前一条路径的公共前缀长度」。
/// 注意它不是朴素的 base-128：每续一个字节都要先 `+1` 再左移 7 位。
function decodeVarint(buf, start, limit) {
  let offset = start;
  let value = buf[offset] & 0x7f;
  while (buf[offset] & 0x80) {
    offset += 1;
    if (offset >= limit) return null;
    value = ((value + 1) << 7) + (buf[offset] & 0x7f);
  }
  return { value, next: offset + 1 };
}

/// 解析出的路径必须是可打印的常规路径。控制字符意味着「把别的东西当成了路径」
/// （例如把 SHA-256 仓库的 32 字节 object name 当成 SHA-1 的 20 字节读，就会出现这种错位）。
function plausible(name) {
  return name.length > 0 && ![...name].some(character => character.codePointAt(0) < 0x20);
}

/// 纯函数：字节流 → `{ok:true, version, paths}` 或 `{ok:false, reason, detail}`。
/// `reason` 恰四类：`truncated` / `bad-magic` / `unsupported-version` / `corrupt`。
export function parseIndex(buf) {
  if (buf.length < HEADER_BYTES) {
    return { ok: false, reason: 'truncated', detail: `header needs ${HEADER_BYTES} bytes, got ${buf.length}` };
  }
  const magic = buf.toString('ascii', 0, 4);
  if (magic !== 'DIRC') {
    return { ok: false, reason: 'bad-magic', detail: `expected "DIRC", got ${JSON.stringify(magic)}` };
  }
  const version = buf.readUInt32BE(4);
  if (version < 2 || version > 4) {
    return { ok: false, reason: 'unsupported-version', detail: `DIRC version ${version} is not one of 2, 3, 4` };
  }
  const count = buf.readUInt32BE(8);
  const paths = [];
  let previous = '';
  let offset = HEADER_BYTES;

  for (let index = 0; index < count; index += 1) {
    const entryStart = offset;
    if (offset + FIXED_BYTES > buf.length) {
      return { ok: false, reason: 'truncated', detail: `entry ${index} header runs past the end of the file` };
    }
    const flags = buf.readUInt16BE(offset + 60);
    offset += FIXED_BYTES;
    const extended = (flags & FLAG_EXTENDED) !== 0;
    if (extended) {
      if (version < 3) {
        return { ok: false, reason: 'bad-magic', detail: `entry ${index} sets the v3-only extended flag in a v${version} index` };
      }
      offset += 2;
    }

    let name;
    if (version === 4) {
      const decoded = decodeVarint(buf, offset, buf.length);
      if (decoded === null) {
        return { ok: false, reason: 'truncated', detail: `entry ${index} has a truncated prefix length` };
      }
      offset = decoded.next;
      const end = buf.indexOf(0, offset);
      if (end < 0) {
        return { ok: false, reason: 'truncated', detail: `entry ${index} name is not NUL-terminated` };
      }
      name = previous.slice(0, decoded.value) + buf.toString('utf8', offset, end);
      offset = end + 1; // v4 的条目不补齐到 8 字节
    } else {
      const end = buf.indexOf(0, offset);
      if (end < 0) {
        return { ok: false, reason: 'truncated', detail: `entry ${index} name is not NUL-terminated` };
      }
      name = buf.toString('utf8', offset, end);
      // 条目总长补齐到 8 字节的倍数（名字的 NUL 结尾算在内）。
      const consumed = FIXED_BYTES + (extended ? 2 : 0) + (end - offset) + 1;
      offset = entryStart + Math.ceil(consumed / 8) * 8;
    }

    if (!plausible(name)) {
      return { ok: false, reason: 'corrupt', detail: `entry ${index} parsed into an implausible path` };
    }
    paths.push(name);
    previous = name;
  }

  return { ok: true, version, paths };
}

/// 读 `<root>/.git/index`。`.git` 是**文件**时（worktree/submodule）跟随它的 `gitdir:` 指针。
export async function readTrackedPaths(root = '.') {
  const dotGit = path.join(root, '.git');
  let info;
  try {
    info = await stat(dotGit);
  } catch (error) {
    return { ok: false, reason: 'missing', detail: `${dotGit} is not readable (${error.code})` };
  }

  let indexPath = path.join(dotGit, 'index');
  if (!info.isDirectory()) {
    let pointer;
    try {
      pointer = (await readFile(dotGit, 'utf8')).trim();
    } catch (error) {
      return { ok: false, reason: 'missing', detail: `${dotGit} is not readable (${error.code})` };
    }
    const match = pointer.match(/^gitdir:\s*(.+)$/);
    if (match === null) {
      return { ok: false, reason: 'missing', detail: `${dotGit} is a file with no "gitdir:" pointer` };
    }
    indexPath = path.resolve(root, match[1].trim(), 'index');
  }

  let buf;
  try {
    buf = await readFile(indexPath);
  } catch (error) {
    return { ok: false, reason: 'missing', detail: `${indexPath} is not readable (${error.code})` };
  }

  const parsed = parseIndex(buf);
  if (!parsed.ok) return { ...parsed, detail: `${indexPath}: ${parsed.detail}` };
  return { ok: true, version: parsed.version, paths: new Set(parsed.paths) };
}

/// 失败原因 → 一句可诊断的话。守卫在**前提缺失**时必须红，且红得说得清是哪一类。
export function describeFailure(failure) {
  return `${failure.reason} (${failure.detail}) — the guard asserts that a schema is read by a file that is in the repository; without the tracked-path set it cannot tell repository content from a stray file in the working tree`;
}
