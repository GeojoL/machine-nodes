# 节点机踩坑与铁律(装机/运维必读)

> 本周 macjol 实战踩出的坑 + 全机群死命令。装新节点(尤其 Mac)或排查通讯/守护问题前先过一遍。

## 死命令(GeojoLu,全机群)

1. **任何代码不允许写死(no-hardcode)**。agent 名单一律枚举 `~/mahaul/agents/*/PERSONA.md`;`CENTER/NODE/PERSONA/cluster_id` 读 `~/.ccp-{node,center,cluster}` + `persona.txt`;会话名 env 覆盖 > 节点名派生 > 兜底。写死致新成员漏纳管(实犯:doorbell 写死名单→新 agent 漏投)。
2. **GitHub 严禁出现 Claude 共作/署名(保密)**。commit 不加 `Co-Authored-By: Claude`、不写 "Generated with Claude Code"、README/PR/注释不提 Claude。推前 `git log --grep -i claude` 须空。
3. **自包含铁律**。依赖/二进制/sidecar 必须在项目内,或配项目内 `scripts/setup-*.sh` 一键重建;禁项目外绝对路径 symlink、禁只存在于 gitignored 产物。

## macOS 节点专坑

4. **launchd 后台进程够不到本地网络**(Local Network Privacy → 连 LAN/ZT 都 `No route to host`;shell/tmux 上下文却通)。
   - 后果:node-agent 握手、`ccp-pull`/`ccp-heartbeat` 等任何"网络腿"若由 launchd 直起,在 Mac 上必失败(本地 file-bus 不受影响)。
   - 解:node-agent 跑在**有网的 tmux 会话** respawn 循环;launchd 只作**看护层**(纯 tmux 操作,会话/窗口没了重建)。见 `nodes-installer.sh` 的 `start_darwin`。
   - 彻底解(可选,需机主 GUI):系统设置>隐私>本地网络 给 python 授权 → 纯 launchd 自恢复(重启自起)。
   - Linux(systemd)无此坑。
5. **launchd StartInterval 定时器会 stall**(长 uptime/负载下),致周期任务静默死(macjol 实测 doorbell 静默 ~12h)。
   - 解:周期任务写**心跳日志**;另一独立 launchd job 查心跳新鲜度、stale 就 `bootout`+`bootstrap` 重载。或改 KeepAlive 常驻 loop。

## 通讯投递坑(本机 AI-AI / najol 线)

6. **身份大小写分裂**(`natty@` vs `Natty@`)致投递大小写敏感子串误判→静默漏投。解:`ccp-resolve-id` 按 roster 规范化 casing(命中回 roster 原文);投递匹配大小写不敏感;`ccp-local-send` 规范化 from/to。
7. **hub-ACK 回环**:跨机 ACK 用文本 `body="ACK:<id>"` 发,会被当普通 peer 消息投递+触发"未读"钩子死循环。解:投递层识别 `body` 单 token `ACK:` 跳过不投。
8. **总线无界增长**(`inbox.jsonl`)→ **游标安全归档**:只裁"所有收件人都已读"的公共前缀,行号游标等量回减,绝不裸 truncate(会破坏行号游标致重发/漏投)。
9. **窗口名匹配大小写不敏感**(`grep -qix`):窗口 `Lobe` 不能因小写 `lobe` 漏判(实犯:doorbell 漏投 Lobe)。

## 自包含待补(TODO)

10. **Windows sidecar bootstrap 缺口**:`setup-sidecars.sh` 只还原 mac;Win 的 ffmpeg/g-chop/python 需 Windows/Linux 构建机重建(Mac 无法自测)。挂 TODO 等构建机(可考虑 DORJOL@doria)。
11. **g-chop 源码回退**:已加 `setup-sidecars.sh --build-gchop`(从树内源码重建,不依赖上个 release)——产物 sha256 与 shipped sidecar 字节一致验证过。
