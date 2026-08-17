use crate::error::Result;
use regex::Regex;

#[derive(Debug, Clone)]
pub enum SvgCommand {
    MoveTo {
        x: f64,
        y: f64,
    },
    LineTo {
        x: f64,
        y: f64,
    },
    Arc {
        rx: f64,
        ry: f64,
        angle: f64,
        large_arc: bool,
        sweep: bool,
        x: f64,
        y: f64,
    },
    ClosePath,
}

fn is_command(token: &str) -> bool {
    token.len() == 1 && token.as_bytes()[0].is_ascii_alphabetic()
}

fn read_number(tokens: &[String], index: &mut usize) -> Option<f64> {
    let token = tokens.get(*index)?;
    if is_command(token) {
        return None;
    }
    *index += 1;
    token.parse().ok()
}

fn read_pair(tokens: &[String], index: &mut usize) -> Option<(f64, f64)> {
    Some((read_number(tokens, index)?, read_number(tokens, index)?))
}

fn add_curve(
    points: &mut Vec<SvgCommand>,
    start: (f64, f64),
    end: (f64, f64),
    controls: &[(f64, f64)],
) {
    // KiCad symbols do not have a cubic path primitive. Approximate curves with
    // short line segments so curved outlines remain visible instead of vanishing.
    const SEGMENTS: usize = 12;
    for step in 1..=SEGMENTS {
        let t = step as f64 / SEGMENTS as f64;
        let point = match controls {
            [control] => {
                let u = 1.0 - t;
                (
                    u * u * start.0 + 2.0 * u * t * control.0 + t * t * end.0,
                    u * u * start.1 + 2.0 * u * t * control.1 + t * t * end.1,
                )
            }
            [first, second] => {
                let u = 1.0 - t;
                (
                    u.powi(3) * start.0
                        + 3.0 * u.powi(2) * t * first.0
                        + 3.0 * u * t.powi(2) * second.0
                        + t.powi(3) * end.0,
                    u.powi(3) * start.1
                        + 3.0 * u.powi(2) * t * first.1
                        + 3.0 * u * t.powi(2) * second.1
                        + t.powi(3) * end.1,
                )
            }
            _ => end,
        };
        points.push(SvgCommand::LineTo {
            x: point.0,
            y: point.1,
        });
    }
}

pub fn parse_svg_path(path: &str) -> Result<Vec<SvgCommand>> {
    let token_re =
        Regex::new(r"[AaCcHhLlMmQqSsTtVvZz]|[-+]?(?:\d*\.\d+|\d+\.?)(?:[eE][-+]?\d+)?").unwrap();
    let tokens: Vec<String> = token_re
        .find_iter(path.trim())
        .map(|match_| match_.as_str().to_string())
        .collect();

    let mut commands = Vec::new();
    let mut index = 0;
    let mut command = None;
    let mut current = (0.0, 0.0);
    let mut subpath_start = current;
    let mut last_cubic_control: Option<(f64, f64)> = None;
    let mut last_quadratic_control: Option<(f64, f64)> = None;

    while index < tokens.len() {
        if is_command(&tokens[index]) {
            command = Some(tokens[index].as_bytes()[0] as char);
            index += 1;
        }
        let Some(active) = command else { break };
        let relative = active.is_ascii_lowercase();
        let upper = active.to_ascii_uppercase();

        match upper {
            'M' => {
                let Some(mut point) = read_pair(&tokens, &mut index) else {
                    command = None;
                    continue;
                };
                if relative {
                    point.0 += current.0;
                    point.1 += current.1;
                }
                current = point;
                subpath_start = point;
                commands.push(SvgCommand::MoveTo {
                    x: point.0,
                    y: point.1,
                });
                last_cubic_control = None;
                last_quadratic_control = None;
                command = Some(if relative { 'l' } else { 'L' });
            }
            'L' => {
                let Some(mut point) = read_pair(&tokens, &mut index) else {
                    command = None;
                    continue;
                };
                if relative {
                    point.0 += current.0;
                    point.1 += current.1;
                }
                current = point;
                commands.push(SvgCommand::LineTo {
                    x: point.0,
                    y: point.1,
                });
                last_cubic_control = None;
                last_quadratic_control = None;
            }
            'H' | 'V' => {
                let Some(value) = read_number(&tokens, &mut index) else {
                    command = None;
                    continue;
                };
                let point = if upper == 'H' {
                    (if relative { current.0 + value } else { value }, current.1)
                } else {
                    (current.0, if relative { current.1 + value } else { value })
                };
                current = point;
                commands.push(SvgCommand::LineTo {
                    x: point.0,
                    y: point.1,
                });
                last_cubic_control = None;
                last_quadratic_control = None;
            }
            'C' | 'S' => {
                let (first, second, mut end) = if upper == 'C' {
                    let Some(first) = read_pair(&tokens, &mut index) else {
                        command = None;
                        continue;
                    };
                    let Some(second) = read_pair(&tokens, &mut index) else {
                        command = None;
                        continue;
                    };
                    let Some(end) = read_pair(&tokens, &mut index) else {
                        command = None;
                        continue;
                    };
                    (first, second, end)
                } else {
                    let first = last_cubic_control
                        .map(|control| (2.0 * current.0 - control.0, 2.0 * current.1 - control.1))
                        .unwrap_or(current);
                    let Some(second) = read_pair(&tokens, &mut index) else {
                        command = None;
                        continue;
                    };
                    let Some(end) = read_pair(&tokens, &mut index) else {
                        command = None;
                        continue;
                    };
                    (first, second, end)
                };
                if relative {
                    let offset = current;
                    if upper == 'C' {
                        end = (end.0 + offset.0, end.1 + offset.1);
                    }
                }
                let (first, second, end) = if relative {
                    let offset = current;
                    if upper == 'C' {
                        (
                            (first.0 + offset.0, first.1 + offset.1),
                            (second.0 + offset.0, second.1 + offset.1),
                            end,
                        )
                    } else {
                        (first, (second.0 + offset.0, second.1 + offset.1), end)
                    }
                } else {
                    (first, second, end)
                };
                add_curve(&mut commands, current, end, &[first, second]);
                current = end;
                last_cubic_control = Some(second);
                last_quadratic_control = None;
            }
            'Q' | 'T' => {
                let control = if upper == 'Q' {
                    let Some(control) = read_pair(&tokens, &mut index) else {
                        command = None;
                        continue;
                    };
                    control
                } else {
                    last_quadratic_control
                        .map(|previous| {
                            (2.0 * current.0 - previous.0, 2.0 * current.1 - previous.1)
                        })
                        .unwrap_or(current)
                };
                let Some(mut end) = read_pair(&tokens, &mut index) else {
                    command = None;
                    continue;
                };
                if relative {
                    let offset = current;
                    end = (end.0 + offset.0, end.1 + offset.1);
                    let control = if upper == 'Q' {
                        (control.0 + offset.0, control.1 + offset.1)
                    } else {
                        control
                    };
                    add_curve(&mut commands, current, end, &[control]);
                    last_quadratic_control = Some(control);
                } else {
                    add_curve(&mut commands, current, end, &[control]);
                    last_quadratic_control = Some(control);
                }
                current = end;
                last_cubic_control = None;
            }
            'A' => {
                let Some(rx) = read_number(&tokens, &mut index) else {
                    command = None;
                    continue;
                };
                let Some(ry) = read_number(&tokens, &mut index) else {
                    command = None;
                    continue;
                };
                let Some(angle) = read_number(&tokens, &mut index) else {
                    command = None;
                    continue;
                };
                let Some(large_arc) = read_number(&tokens, &mut index) else {
                    command = None;
                    continue;
                };
                let Some(sweep) = read_number(&tokens, &mut index) else {
                    command = None;
                    continue;
                };
                let Some(mut end) = read_pair(&tokens, &mut index) else {
                    command = None;
                    continue;
                };
                if relative {
                    end.0 += current.0;
                    end.1 += current.1;
                }
                commands.push(SvgCommand::Arc {
                    rx,
                    ry,
                    angle,
                    large_arc: large_arc != 0.0,
                    sweep: sweep != 0.0,
                    x: end.0,
                    y: end.1,
                });
                current = end;
                last_cubic_control = None;
                last_quadratic_control = None;
            }
            'Z' => {
                commands.push(SvgCommand::ClosePath);
                current = subpath_start;
                command = None;
                last_cubic_control = None;
                last_quadratic_control = None;
            }
            _ => {
                command = None;
            }
        }
    }

    Ok(commands)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_compact_relative_lines() {
        let commands = parse_svg_path("m10,20 l5,0 0,5 z").unwrap();
        assert_eq!(commands.len(), 4);
        assert!(matches!(commands[1], SvgCommand::LineTo { x, y } if x == 15.0 && y == 20.0));
    }

    #[test]
    fn parses_horizontal_vertical_and_curves() {
        let commands = parse_svg_path("M0 0 H10 V10 C10 20 20 20 20 10").unwrap();
        assert!(commands.len() > 10);
    }

    #[test]
    fn parses_arc_with_comma_separators() {
        let commands = parse_svg_path("M0,0 A10,10 0 1 0 20,20").unwrap();
        assert_eq!(commands.len(), 2);
        assert!(matches!(
            commands[1],
            SvgCommand::Arc {
                large_arc: true,
                sweep: false,
                ..
            }
        ));
    }
}
