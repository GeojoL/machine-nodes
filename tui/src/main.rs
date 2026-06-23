// union-tui — The Union 中心机/节点机控制台 TUI(v1)。
// v1 面板:Messages(消息中心,读总线调试) / Personas(人格列表+查看,e 调 $EDITOR 编辑) / Status(状态总览)。
// 无头自检:`union-tui --dump`。后续迭代:加人格写入、服务控制(守护/门铃/升级)、center 模式聚合节点。
// 全 Rust 自包含;数据源 = ~/.ccp-inbox/{inbox,local}.jsonl(总线)+ $AGENTS_ROOT/*/PERSONA.md(人格)。

use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap};
use ratatui::Frame;

// ---------- 数据模型 ----------
#[derive(Clone)]
struct Msg {
    ts: String,
    from: String,
    to: String,
    kind: String,
    body: String,
    via: &'static str, // "najol" | "local"
}

#[derive(Clone)]
struct Persona {
    name: String,
    identity: String,
    level: String,
    role: String,
    window: String,
    home: String,
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn agents_root() -> PathBuf {
    std::env::var("AGENTS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home().join("mahaul/agents"))
}

fn node_name() -> String {
    fs::read_to_string(home().join(".ccp-node"))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "?".to_string())
}

// 读一个 jsonl 总线文件 → Vec<Msg>
fn read_jsonl(path: PathBuf, via: &'static str) -> Vec<Msg> {
    let mut out = Vec::new();
    if let Ok(content) = fs::read_to_string(&path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                let get = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
                let mut body = get("body");
                if body.is_empty() {
                    // ack 等无 body 的,展示 kind/ack 摘要
                    if let Some(a) = v.get("ack").and_then(|x| x.as_str()) {
                        body = format!("(ack {})", a);
                    }
                }
                out.push(Msg {
                    ts: get("ts"),
                    from: get("from"),
                    to: get("to"),
                    kind: get("kind"),
                    body,
                    via,
                });
            }
        }
    }
    out
}

fn read_bus() -> Vec<Msg> {
    let base = home().join(".ccp-inbox");
    let mut msgs = read_jsonl(base.join("inbox.jsonl"), "najol");
    msgs.extend(read_jsonl(base.join("local.jsonl"), "local"));
    msgs.sort_by(|a, b| a.ts.cmp(&b.ts)); // 按时间升序
    msgs
}

fn parse_persona(path: &PathBuf) -> Option<Persona> {
    let content = fs::read_to_string(path).ok()?;
    let field = |k: &str| -> String {
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix(&format!("{}:", k)) {
                return rest.trim().to_string();
            }
        }
        String::new()
    };
    let name = field("name");
    if name.is_empty() {
        return None;
    }
    Some(Persona {
        name,
        identity: field("identity"),
        level: field("level"),
        role: field("role"),
        window: field("window"),
        home: field("home"),
    })
}

fn read_personas() -> Vec<Persona> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(agents_root()) {
        for e in entries.flatten() {
            let pf = e.path().join("PERSONA.md");
            if pf.is_file() {
                if let Some(p) = parse_persona(&pf) {
                    out.push(p);
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

// ---------- App 状态 ----------
#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Messages,
    Personas,
    Status,
}

struct App {
    tab: Tab,
    msgs: Vec<Msg>,
    personas: Vec<Persona>,
    msg_state: ListState,
    persona_state: ListState,
    only_to_me: bool,
    quit: bool,
}

impl App {
    fn new() -> Self {
        let mut a = App {
            tab: Tab::Messages,
            msgs: read_bus(),
            personas: read_personas(),
            msg_state: ListState::default(),
            persona_state: ListState::default(),
            only_to_me: false,
            quit: false,
        };
        // 默认选中最后一条消息(最新)
        let n = a.filtered_msgs().len();
        if n > 0 {
            a.msg_state.select(Some(n - 1));
        }
        if !a.personas.is_empty() {
            a.persona_state.select(Some(0));
        }
        a
    }

    fn me(&self) -> String {
        format!("Mahaul@{}", node_name())
    }

    fn filtered_msgs(&self) -> Vec<Msg> {
        let me = self.me();
        self.msgs
            .iter()
            .filter(|m| !self.only_to_me || m.to == me || m.to == "broadcast")
            .cloned()
            .collect()
    }

    fn reload(&mut self) {
        self.msgs = read_bus();
        self.personas = read_personas();
    }

    fn next(&mut self) {
        match self.tab {
            Tab::Messages => {
                let n = self.filtered_msgs().len();
                step(&mut self.msg_state, n, 1);
            }
            Tab::Personas => step(&mut self.persona_state, self.personas.len(), 1),
            Tab::Status => {}
        }
    }
    fn prev(&mut self) {
        match self.tab {
            Tab::Messages => {
                let n = self.filtered_msgs().len();
                step(&mut self.msg_state, n, -1);
            }
            Tab::Personas => step(&mut self.persona_state, self.personas.len(), -1),
            Tab::Status => {}
        }
    }
}

fn step(state: &mut ListState, len: usize, dir: i32) {
    if len == 0 {
        return;
    }
    let cur = state.selected().unwrap_or(0) as i32;
    let next = (cur + dir).rem_euclid(len as i32);
    state.select(Some(next as usize));
}

// ---------- 无头自检 ----------
fn dump() {
    let msgs = read_bus();
    let personas = read_personas();
    println!("=== union-tui --dump (node={}) ===", node_name());
    println!("\n[消息中心] 总线消息 {} 条,末 10 条:", msgs.len());
    for m in msgs.iter().rev().take(10).rev() {
        let body1: String = m.body.lines().next().unwrap_or("").chars().take(70).collect();
        println!(
            "  [{}|{}] {} -> {} ({}): {}",
            m.via, m.ts, m.from, m.to, m.kind, body1
        );
    }
    println!("\n[人格] {} 个:", personas.len());
    for p in &personas {
        println!(
            "  {} | level={} window={} | {}",
            p.identity, p.level, p.window, p.role
        );
    }
}

// ---------- UI ----------
fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // tabs
        Constraint::Min(0),    // body
        Constraint::Length(1), // help
    ])
    .split(f.area());

    let titles = ["Messages", "Personas", "Status"];
    let sel = match app.tab {
        Tab::Messages => 0,
        Tab::Personas => 1,
        Tab::Status => 2,
    };
    let tabs = Tabs::new(titles.to_vec())
        .select(sel)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" The Union · {} ", node_name())),
        )
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan));
    f.render_widget(tabs, chunks[0]);

    match app.tab {
        Tab::Messages => render_messages(f, app, chunks[1]),
        Tab::Personas => render_personas(f, app, chunks[1]),
        Tab::Status => render_status(f, app, chunks[1]),
    }

    let help = match app.tab {
        Tab::Personas => " q退出  Tab切面板  ↑↓选择  e用$EDITOR编辑人格  r刷新 ",
        Tab::Messages => " q退出  Tab切面板  ↑↓滚动  f仅看发我的  r刷新 ",
        _ => " q退出  Tab切面板  r刷新 ",
    };
    f.render_widget(
        Paragraph::new(help).style(Style::default().fg(Color::DarkGray)),
        chunks[2],
    );
}

fn render_messages(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let cols = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);
    let fm = app.filtered_msgs();
    let items: Vec<ListItem> = fm
        .iter()
        .map(|m| {
            let ts = m.ts.chars().skip(5).take(11).collect::<String>(); // MM-DDThh:mm 粗略
            let color = if m.via == "local" { Color::Green } else { Color::Yellow };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:11} ", ts), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{:<16}", short(&m.from, 16)), Style::default().fg(color)),
                Span::raw(format!("→{}", short(&m.to, 14))),
            ]))
        })
        .collect();
    let title = format!(
        " 消息中心 {}{} ",
        fm.len(),
        if app.only_to_me { " (仅发我的)" } else { "" }
    );
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, cols[0], &mut app.msg_state);

    // 详情
    let detail = app
        .msg_state
        .selected()
        .and_then(|i| fm.get(i))
        .map(|m| {
            format!(
                "时间: {}\n来源: {} ({})\n收件: {}\n类型: {}\n\n{}",
                m.ts, m.from, m.via, m.to, m.kind, m.body
            )
        })
        .unwrap_or_else(|| "(无消息)".to_string());
    f.render_widget(
        Paragraph::new(detail)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(" 详情 ")),
        cols[1],
    );
}

fn render_personas(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let cols = Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).split(area);
    let items: Vec<ListItem> = app
        .personas
        .iter()
        .map(|p| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:<18}", p.identity), Style::default().fg(Color::Cyan)),
                Span::styled(format!("[{}]", p.level), Style::default().fg(Color::Magenta)),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" 人格 {} ", app.personas.len())),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, cols[0], &mut app.persona_state);

    let detail = app
        .persona_state
        .selected()
        .and_then(|i| app.personas.get(i))
        .map(|p| {
            format!(
                "身份: {}\n级别: {}\n窗口: {}\n家目录: {}\n\n职责:\n{}\n\n(按 e 用 $EDITOR/nvim 编辑 PERSONA.md)",
                p.identity, p.level, p.window, p.home, p.role
            )
        })
        .unwrap_or_else(|| "(无人格;用 add-persona 添加)".to_string());
    f.render_widget(
        Paragraph::new(detail)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(" 详情 ")),
        cols[1],
    );
}

fn render_status(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let me = app.me();
    let to_me = app.msgs.iter().filter(|m| m.to == me).count();
    let text = format!(
        "节点: {}\n本机身份: {}\n\n人格数: {}\n总线消息: {} (najol {} + local {})\n发给我的: {}\n\n[迭代中] 服务控制(守护/门铃/升级)、加人格写入、center 聚合 —— 后续版本。",
        node_name(),
        me,
        app.personas.len(),
        app.msgs.len(),
        app.msgs.iter().filter(|m| m.via == "najol").count(),
        app.msgs.iter().filter(|m| m.via == "local").count(),
        to_me,
    );
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(" 状态总览 ")),
        area,
    );
}

fn short(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        s.chars().take(n).collect()
    } else {
        s.to_string()
    }
}

// 编辑选中人格的 PERSONA.md(挂起 TUI → $EDITOR → 恢复)
fn edit_persona(app: &App) -> io::Result<()> {
    if let Some(p) = app.persona_state.selected().and_then(|i| app.personas.get(i)) {
        let pf = PathBuf::from(&p.home).join("PERSONA.md");
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());
        ratatui::restore();
        let _ = Command::new(&editor).arg(&pf).status();
        // 调用方负责重新 init + reload
    }
    Ok(())
}

fn main() -> io::Result<()> {
    if std::env::args().any(|a| a == "--dump") {
        dump();
        return Ok(());
    }
    let mut terminal = ratatui::init();
    let mut app = App::new();
    while !app.quit {
        terminal.draw(|f| ui(f, &mut app))?;
        if event::poll(Duration::from_millis(500))? {
            if let Event::Key(k) = event::read()? {
                if k.kind != KeyEventKind::Press {
                    continue;
                }
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => app.quit = true,
                    KeyCode::Tab => {
                        app.tab = match app.tab {
                            Tab::Messages => Tab::Personas,
                            Tab::Personas => Tab::Status,
                            Tab::Status => Tab::Messages,
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Up | KeyCode::Char('k') => app.prev(),
                    KeyCode::Char('f') if app.tab == Tab::Messages => {
                        app.only_to_me = !app.only_to_me;
                        let n = app.filtered_msgs().len();
                        app.msg_state.select(if n > 0 { Some(n - 1) } else { None });
                    }
                    KeyCode::Char('r') => app.reload(),
                    KeyCode::Char('e') if app.tab == Tab::Personas => {
                        edit_persona(&app)?;
                        terminal = ratatui::init();
                        app.reload();
                    }
                    _ => {}
                }
            }
        }
    }
    ratatui::restore();
    Ok(())
}
