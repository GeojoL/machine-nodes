#!/usr/bin/env bash
# machine-nodes · nodes-installer — 在【节点机】上一键装:握手 agent + 随机命名人格 + 加入 center 握手。
# 零依赖(python3 stdlib)。只需对 center 出站 HTTP(不要求 center 能 ssh 进来)。幂等可重跑。
# 用法(在节点机上跑):
#   curl -fsSL <raw>/nodes-installer.sh | CENTER=http://<center-ip>:8770 bash
#   或:  CENTER=http://192.168.50.50:8770 ./nodes-installer.sh
# 可选 env: NODE=<节点名,默认hostname>  PERSONA=<指定人格,默认随机>  HS_INTERVAL=60  HS_DURATION=43200
set -euo pipefail
CENTER="${CENTER:-}"; [ -n "$CENTER" ] || { echo "需要 CENTER 环境变量,如 CENTER=http://192.168.50.50:8770"; exit 2; }
CENTER="${CENTER%/}"
NODE="${NODE:-$(hostname | cut -d. -f1)}"
INTERVAL="${HS_INTERVAL:-60}"; DURATION="${HS_DURATION:-43200}"
RAW="${MN_RAW:-https://raw.githubusercontent.com/GeojoL/the-union/main}"
BIN="$HOME/.local/bin"; NHOME="$HOME/machine-nodes-node"; mkdir -p "$BIN" "$NHOME"
say(){ printf '\033[35m[nodes-installer]\033[0m %s\n' "$*"; }
command -v python3 >/dev/null || { echo "需要 python3"; exit 1; }

# 1) 取 node-agent.py(本地仓库优先,否则远程)
SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" 2>/dev/null && pwd || echo .)"
if [ -f "$SRC_DIR/node/node-agent.py" ]; then cp "$SRC_DIR/node/node-agent.py" "$BIN/mn-node-agent.py"; say "用本地 node-agent.py"
else say "远程拉 node-agent.py"; curl -fsSL "$RAW/node/node-agent.py" -o "$BIN/mn-node-agent.py"; fi
chmod +x "$BIN/mn-node-agent.py"

# 2) 探活 center
curl -fsS -m6 "$CENTER/hs/status" >/dev/null 2>&1 || { echo "连不上 center $CENTER —— 查中心是否在跑、端口/防火墙"; exit 1; }
say "center 可达: $CENTER"

# 3) 随机命名人格(带唯一性:撞 center 已有名就重摇)
PERSONA="${PERSONA:-}"
if [ -z "$PERSONA" ]; then
  ADJ=(swift calm bright bold quiet keen lucid amber jade onyx coral slate vivid noble brisk); NOUN=(otter lynx heron raven ibis koi crane fox marten egret tern vole stoat finch wren)
  EXIST="$(curl -fsS -m6 "$CENTER/hs/status" 2>/dev/null | python3 -c 'import sys,json;d=json.load(sys.stdin);print(" ".join(v.get("persona","") for v in d.values()))' 2>/dev/null || echo)"
  for _ in 1 2 3 4 5 6 7 8; do
    r=$(( (RANDOM) % ${#ADJ[@]} )); s=$(( (RANDOM) % ${#NOUN[@]} )); h=$(printf '%02x' $((RANDOM%256)))
    cand="${ADJ[$r]}-${NOUN[$s]}-$h"
    case " $EXIST " in *" $cand "*) continue;; *) PERSONA="$cand"; break;; esac
  done
  PERSONA="${PERSONA:-node-$(date +%s | tail -c5)}"
fi
say "人格(随机命名)= $PERSONA   节点 = $NODE"
# 人格身份文件(供后续起 claude 会话用;本 installer 只建身份+登记,起活会话是可选的)
cat > "$NHOME/persona.txt" <<EOF
persona=$PERSONA
node=$NODE
center=$CENTER
machine=$(uname -s) $(uname -m)
created=$(date '+%Y-%m-%dT%H:%M:%S')
role=machine-nodes 节点级人格(随机命名);职责=本机节点 agent + 与 center 读确认握手
EOF

# 3b) 装【通讯身份·防冒充】toolkit(Mahaul@macjol 实战 + najol B+ 标准;全节点同律)
#     原理:ccp-send/ccp-pull 未设 CCP_ID 时,按当前 tmux 窗口名派生身份 capitalize(窗名)@<节点名>,
#     再对本机 roster 校验;拿不到窗名/不在名单 → fail-loud 拒发,绝不回退默认身份冒充别人。
say "装通讯身份防冒充 toolkit(ccp-resolve-id + ~/.ccp-node + roster)"
if [ -f "$SRC_DIR/node/ccp-resolve-id" ]; then cp "$SRC_DIR/node/ccp-resolve-id" "$BIN/ccp-resolve-id"
else curl -fsSL "$RAW/node/ccp-resolve-id" -o "$BIN/ccp-resolve-id"; fi
chmod +x "$BIN/ccp-resolve-id"
printf '%s\n' "$NODE" > "$HOME/.ccp-node"          # 节点名(不裸 hostname:本机 hostname 可能≠节点名)
mkdir -p "$HOME/.ccp-inbox"
if [ ! -f "$HOME/.ccp-inbox/roster" ]; then
  cat > "$HOME/.ccp-inbox/roster" <<RO
# 本机通讯参与者名单(防冒充校验源)。每行一个完整身份 = Capitalize(tmux窗口名)@$NODE
# ccp-send/ccp-pull 未设 CCP_ID 时按当前 tmux 窗口名派生身份,必须在本名单内才放行(否则 fail-loud 拒发)。
# 正式 AI 身份权威在根 AI-PROTOCOL §1(变动报根);本机 dev persona 留本地即可。
# 例(按你的窗口名填,删本注释行):
# $PERSONA@$NODE
RO
  say "已建 roster 脚手架 ~/.ccp-inbox/roster(请按本机窗口名填入参与者身份)"
fi
# ⚠ caveat:把 ccp-send/ccp-pull 默认改 fail-loud 后,【非 tmux 且没设 CCP_ID 的 launchd/cron 调用者会被拒】。
#   本机若有此类自动调用者(如投递门铃内部 ccp-pull),务必在其 plist/unit 环境里显式设 CCP_ID,否则通讯故障。

# 4) 起 node-agent 服务(systemd / launchd / nohup)
OS="$(uname -s)"; PY="$(command -v python3)"
start_systemd(){
  local unit="$HOME/.config/systemd/user/machine-nodes-node.service"; mkdir -p "$(dirname "$unit")"
  cat > "$unit" <<EOF
[Unit]
Description=machine-nodes node-agent ($PERSONA)
After=network.target
[Service]
Environment=CENTER=$CENTER
Environment=NODE=$NODE
Environment=PERSONA=$PERSONA
Environment=HS_INTERVAL=$INTERVAL
Environment=HS_DURATION=$DURATION
ExecStart=$PY $BIN/mn-node-agent.py
Restart=always
RestartSec=5
[Install]
WantedBy=default.target
EOF
  systemctl --user daemon-reload; systemctl --user enable --now machine-nodes-node.service
  say "systemd user service machine-nodes-node 已起"
}
start_darwin(){
  # ★macOS:launchd 后台进程够不到本地网络(Local Network Privacy → 连 LAN/ZT center EHOSTUNREACH;
  #   macjol 实测:shell/tmux 上下文 curl+urllib 都通,launchd 直起必失败)。故 node-agent 的【握手腿】
  #   必须跑在【有网的 tmux 上下文】;launchd 仅作【看护层】(纯 tmux 操作,不碰网络)。
  #   ⚠ tmux 会话须从【有网交互上下文】首次建立(本 installer 即此上下文);重启/登出致 tmux server 整个没了后,
  #     需从交互终端重跑本 installer 重建(或给 python 授 System Settings>Privacy>Local Network 后才可纯 launchd)。
  command -v tmux >/dev/null || { echo "macOS 节点需 tmux(brew install tmux):node-agent 须跑有网 tmux 上下文"; exit 1; }
  local TB; TB="$(command -v tmux)"
  local SESS="${MN_TMUX_SESSION:-mn-node}" WIN=nodeagent
  local LOOP="while true; do CENTER=$CENTER NODE=$NODE PERSONA=$PERSONA HS_INTERVAL=$INTERVAL HS_DURATION=$DURATION $PY $BIN/mn-node-agent.py; echo \"[respawn \$(date +%H:%M:%S)]\"; sleep 5; done"
  "$TB" has-session -t "$SESS" 2>/dev/null || "$TB" new-session -d -s "$SESS" -n "$WIN"
  "$TB" list-windows -t "$SESS" -F '#{window_name}' 2>/dev/null | grep -qix "$WIN" || "$TB" new-window -d -t "$SESS" -n "$WIN"
  "$TB" send-keys -t "$SESS:$WIN" -l "$LOOP"; "$TB" send-keys -t "$SESS:$WIN" Enter
  say "node-agent 起在 tmux $SESS:$WIN(有网上下文)respawn 循环"
  # 看护脚本(纯 tmux 操作,不碰网络):会话/窗口没了就重建+重起循环(仅在刚重建时起,避免 respawn 间隙双起)
  local guard="$BIN/mn-node-guard.sh"
  cat > "$guard" <<GUARD
#!/usr/bin/env bash
set -uo pipefail
TB="\$(command -v tmux || echo $TB)"; SESS="$SESS"; WIN="$WIN"; NEW=
LOOP='$LOOP'
"\$TB" has-session -t "\$SESS" 2>/dev/null || { "\$TB" new-session -d -s "\$SESS" -n "\$WIN"; NEW=1; }
"\$TB" list-windows -t "\$SESS" -F '#{window_name}' 2>/dev/null | grep -qix "\$WIN" || { "\$TB" new-window -d -t "\$SESS" -n "\$WIN"; NEW=1; }
[ -n "\$NEW" ] && { "\$TB" send-keys -t "\$SESS:\$WIN" -l "\$LOOP"; "\$TB" send-keys -t "\$SESS:\$WIN" Enter; }
GUARD
  chmod +x "$guard"
  local pl="$HOME/Library/LaunchAgents/com.geojol.mn-node-guard.plist"; mkdir -p "$(dirname "$pl")"
  cat > "$pl" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
 <key>Label</key><string>com.geojol.mn-node-guard</string>
 <key>ProgramArguments</key><array><string>/bin/bash</string><string>$guard</string></array>
 <key>EnvironmentVariables</key><dict><key>PATH</key><string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin</string></dict>
 <key>StartInterval</key><integer>120</integer><key>RunAtLoad</key><true/>
</dict></plist>
EOF
  launchctl bootout "gui/$(id -u)/com.geojol.mn-node-guard" 2>/dev/null || launchctl unload "$pl" 2>/dev/null || true
  launchctl bootstrap "gui/$(id -u)" "$pl" 2>/dev/null || launchctl load "$pl" 2>/dev/null || true
  say "看护 launchd com.geojol.mn-node-guard 已起(纯 tmux 操作:会话/窗口没了重建+重起 respawn)"
}
start_nohup(){
  pkill -f "mn-node-agent.py" 2>/dev/null || true; sleep 0.5
  CENTER=$CENTER NODE=$NODE PERSONA=$PERSONA HS_INTERVAL=$INTERVAL HS_DURATION=$DURATION \
    setsid nohup "$PY" "$BIN/mn-node-agent.py" >> "$NHOME/node.out" 2>&1 < /dev/null & disown 2>/dev/null || true
  say "nohup 兜底起"
}
case "$OS" in
  Linux)  if command -v systemctl >/dev/null && systemctl --user show-environment >/dev/null 2>&1; then start_systemd; else start_nohup; fi ;;
  Darwin) start_darwin ;;
  *)      start_nohup ;;
esac

# 5) 验证已登记 + 开始握手
sleep 4
if curl -fsS -m6 "$CENTER/hs/status" 2>/dev/null | python3 -c "import sys,json;d=json.load(sys.stdin);exit(0 if '$NODE' in d else 1)" 2>/dev/null; then
  say "✅ 节点 $NODE(人格 $PERSONA)已登记 + 握手已开始。"
  say "本机看: cat $NHOME/handshake.stat   |  中心看: curl -s $CENTER/ "
else
  echo "[nodes-installer] ⚠ 起后未在 center 看到登记,查 $NHOME/handshake.log"; exit 1
fi
