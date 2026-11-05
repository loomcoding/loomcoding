//! Boceto del dashboard limpio: tres disposiciones con el mismo escenario y un muñeco braille cuya
//! postura dice qué hace el agente. Es un mock de diseño: los datos son de mentira.

use crate::demo::Demo;
use loomcoding_cloth::widgets::mascot::{mascot_frame, Pose};
use loomcoding_cloth::widgets::{Anim, Theme};
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Disposición del dashboard.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// A · sin panel, estado en una línea.
    Minimal,
    /// B · panel estrecho sin cajas ni separadores (por defecto).
    Rail,
    /// C · tarjeta de agentes sobre el compositor.
    Cards,
}

const POSES: [(Pose, &str, &str); 6] = [
    (Pose::Think, "pensando", "razonando…"),
    (
        Pose::Run,
        "ejecutando",
        "bash · cargo test -p loomcoding-core",
    ),
    (Pose::Hunt, "buscando", "grep · orientation_for"),
    (Pose::Wait, "esperando", "te toca: ¿sigo con la ronda 3?"),
    (Pose::Done, "terminado", "ronda 2/3 · check verde"),
    (Pose::Fail, "fallido", "subagent reached its step limit"),
];

pub struct CleanDashboardDemo {
    variant: Variant,
    pose: usize,
    tick: u64,
    mascot: bool,
}

impl CleanDashboardDemo {
    pub fn new(variant: Variant) -> Self {
        Self {
            variant,
            pose: 1,
            tick: 0,
            mascot: true,
        }
    }
}

impl Demo for CleanDashboardDemo {
    fn title(&self) -> &str {
        "Dashboard limpio"
    }

    fn hint(&self) -> &str {
        "1-6 postura del muñeco · m muñeco on/off · (pensar, ejecutar, buscar, esperar, listo, fallo)"
    }

    fn fullscreen(&self) -> bool {
        true
    }

    fn tick(&mut self, _anim: &Anim) {
        self.tick = self.tick.wrapping_add(1);
    }

    fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char(c @ '1'..='6') => self.pose = (c as usize) - ('1' as usize),
            KeyCode::Char('m') => self.mascot = !self.mascot,
            _ => return false,
        }
        true
    }

    fn render(&mut self, f: &mut Frame, area: Rect, theme: &Theme) {
        let s = &theme.styles;
        let (pose, pose_name, doing) = POSES[self.pose];
        let bar = |frac: f32| {
            let full = (frac * 8.0).round() as usize;
            Line::from(vec![
                Span::styled("█".repeat(full), s.accent),
                Span::styled("░".repeat(8 - full), s.dim),
            ])
        };

        // Cabecera de una línea: identidad a la izquierda, contexto y flujo a la derecha.
        let head_left = Line::from(vec![
            Span::styled(" loom", s.accent.add_modifier(Modifier::BOLD)),
            Span::styled(" · Ornith-1.5-9B · low", s.dim),
        ]);
        let mut head_right = vec![Span::styled("ctx 12k/64k ", s.dim)];
        head_right.extend(bar(0.19).spans);
        if self.variant != Variant::Rail {
            head_right.push(Span::styled("  gauntlet #3", s.accent));
            head_right.push(Span::styled(" · ronda 2/3 ", s.dim));
        } else {
            head_right.push(Span::raw(" "));
        }

        let strip_h = if self.mascot { 3 } else { 1 };
        let card_h = if self.variant == Variant::Cards { 6 } else { 0 };
        let rows = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(card_h),
            Constraint::Length(strip_h),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

        f.render_widget(Paragraph::new(head_left), rows[0]);
        f.render_widget(
            Paragraph::new(Line::from(head_right)).right_aligned(),
            rows[0],
        );

        // Cuerpo: transcript sin cajas; en B, panel a la derecha sin separador.
        let (chat, panel) = if self.variant == Variant::Rail && area.width >= 80 {
            let [a, b] =
                Layout::horizontal([Constraint::Min(40), Constraint::Length(33)]).areas(rows[1]);
            (a, Some(b))
        } else {
            (rows[1], None)
        };
        f.render_widget(Paragraph::new(transcript_lines(theme)), chat);
        if let Some(panel) = panel {
            f.render_widget(Paragraph::new(panel_lines(theme)), panel);
        }

        if self.variant == Variant::Cards {
            f.render_widget(Paragraph::new(card_lines(theme, rows[2].width)), rows[2]);
        }

        // Franja del muñeco: postura a la izquierda, qué hace a la derecha.
        if self.mascot {
            let [mascot_area, text_area] =
                Layout::horizontal([Constraint::Length(9), Constraint::Min(1)]).areas(rows[3]);
            let [top, bottom] = mascot_frame(pose, self.tick);
            let paint = match pose {
                Pose::Fail => s.error,
                Pose::Done => s.success,
                _ => s.accent,
            };
            let hunt = if pose == Pose::Hunt { "  ⌕" } else { "" };
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(format!(" {top}{hunt}"), paint)),
                    Line::from(Span::styled(format!(" {bottom}"), paint)),
                ]),
                mascot_area,
            );
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(pose_name, s.accent.add_modifier(Modifier::BOLD)),
                        Span::styled(format!("  {doing}"), s.muted),
                    ]),
                ]),
                text_area,
            );
        } else {
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(" ◌ ", s.dim),
                    Span::styled(format!("{pose_name} · {doing}"), s.dim),
                ])),
                rows[3],
            );
        }

        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" ❯ ", s.accent),
                Span::styled("Describe lo que quieres construir…", s.dim),
            ])),
            rows[4],
        );
        let keys = if self.variant == Variant::Cards {
            " enter envía · ctrl+o pliega · ctrl+b tarjeta · ? atajos"
        } else {
            " enter envía · ctrl+o pliega · ctrl+b panel · ? atajos"
        };
        f.render_widget(Paragraph::new(Span::styled(keys, s.dim)), rows[5]);
    }
}

fn transcript_lines(theme: &Theme) -> Vec<Line<'static>> {
    let s = &theme.styles;
    let tool = |glyph: &'static str, glyph_style, name: &'static str, rest: &'static str| {
        Line::from(vec![
            Span::styled(format!(" {glyph} "), glyph_style),
            Span::styled(format!("{name:<9}"), s.muted),
            Span::styled(rest, s.dim),
        ])
    };
    vec![
        Line::from(vec![
            Span::styled(" › ", s.accent),
            Span::styled(
                "modo gauntlet: construye la orientación previa del plan",
                s.default.add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" ◌ ", s.dim),
            Span::styled("gauntlet #3 · constructor trabajando · ronda 2/3", s.dim),
        ]),
        Line::from(""),
        tool(
            "✓",
            s.success,
            "view",
            "docs/plans/…contexto-estructural.md",
        ),
        tool("✓", s.success, "write", "config/orientation.rs"),
        tool(
            "✓",
            s.success,
            "bash",
            "cargo test -p loomcoding-core   0.9s",
        ),
        Line::from(vec![
            Span::styled(" ✗ ", s.error),
            Span::styled("inspector  ", s.muted),
            Span::styled("failed", s.error),
            Span::styled(" · 3/3 pasos · ↑7.6k ↓42", s.dim),
        ]),
        Line::from(Span::styled(
            "     └ subagent reached its step limit without a final response",
            s.dim,
        )),
        Line::from(""),
        Line::from(
            " He creado `ContextConfig` aparte de `LoomcodingConfig`, como pide la Tarea 6.",
        ),
        Line::from(" Falta enganchar `orientation_for` en `run_with_origin`."),
    ]
}

fn panel_lines(theme: &Theme) -> Vec<Line<'static>> {
    let s = &theme.styles;
    let head =
        |t: &'static str| Line::from(Span::styled(t, s.default.add_modifier(Modifier::BOLD)));
    let agent = |g: &'static str, st, name: &'static str, t: &'static str| {
        Line::from(vec![
            Span::styled(format!("{g} "), st),
            Span::raw(format!("{name:<13}")),
            Span::styled(t, s.dim),
        ])
    };
    vec![
        head("Agentes"),
        agent("◐", s.accent, "constructor", "3m12s"),
        agent("○", s.dim, "critico", "cola"),
        agent("✓", s.success, "explorer", "40s"),
        agent("✗", s.error, "inspector", "12s"),
        Line::from(""),
        head("Tokens"),
        Line::from(vec![
            Span::styled("entrada ", s.dim),
            Span::raw("12.3k  "),
            Span::styled("salida ", s.dim),
            Span::raw("1.1k"),
        ]),
        Line::from(vec![
            Span::styled("caché   ", s.dim),
            Span::raw("8.0k leídos"),
        ]),
        Line::from(""),
        Line::from(Span::styled("MCP – · LSP rust-analyzer", s.dim)),
    ]
}

fn card_lines(theme: &Theme, width: u16) -> Vec<Line<'static>> {
    let s = &theme.styles;
    let width = usize::from(width);
    let row = |g: &'static str, st, name: &'static str, task: &'static str, meta: &'static str| {
        let left = format!("  {g}  {name:<12} {task:<24}");
        let pad = width.saturating_sub(left.chars().count() + meta.chars().count() + 1);
        Line::from(vec![
            Span::styled(format!("  {g}  "), st),
            Span::raw(format!("{name:<12} ")),
            Span::styled(format!("{task:<24}"), s.muted),
            Span::raw(" ".repeat(pad)),
            Span::styled(format!("{meta} "), s.dim),
        ])
    };
    let title = "▾ plegar ";
    let gap = width.saturating_sub(10 + title.chars().count());
    vec![
        Line::from(vec![
            Span::styled(" ❀ ", s.accent),
            Span::styled("Agentes", s.default.add_modifier(Modifier::BOLD)),
            Span::raw(" ".repeat(gap)),
            Span::styled(title, s.dim),
        ]),
        row(
            "◐",
            s.accent,
            "constructor",
            "orientation.rs",
            "9B · low     4.2k   3m12s",
        ),
        row(
            "○",
            s.dim,
            "critico",
            "en cola",
            "9B · none      —       — ",
        ),
        row(
            "✓",
            s.success,
            "explorer",
            "mapa de config",
            "9B · none   1.1k     40s",
        ),
        row(
            "✗",
            s.error,
            "inspector",
            "sin respuesta final",
            "9B · low    7.6k     12s",
        ),
    ]
}
