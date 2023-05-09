mod perf;

use async_std::channel::{unbounded, Receiver, Sender};
use async_std::task;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use project_root::get_project_root;
use std::cmp::{max, min, Ordering};
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::{fmt, io, thread, time::Duration, time::Instant};
use tui::widgets::canvas::{Canvas, Line, Map, MapResolution};
use tui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    symbols,
    text::{Span, Spans},
    widgets::*,
    Terminal,
};
extern crate systemstat;
use systemstat::{saturating_sub_bytes, ByteSize, Platform, System};

use crate::perf::{PerfData, PerfLog};
use ctm::*;

#[derive(Copy, Clone, Debug, PartialEq)]
enum MenuItem {
    Running,
    Finished,
    Stats,
    Map,
    Help,
}

impl From<MenuItem> for usize {
    fn from(input: MenuItem) -> usize {
        match input {
            MenuItem::Running => 0,
            MenuItem::Finished => 1,
            MenuItem::Stats => 2,
            MenuItem::Map => 3,
            MenuItem::Help => 4,
        }
    }
}

enum UIEvent<I> {
    Input(I),
    Tick,
}

/// Returns the three widgets required for the main page:
/// 1. A command list that contains all the running or finished commands.
/// 2. A status window that shows the running time of the selected command.
/// 3. An output window showing the output of the selected command.
///
/// Returns:
///     List      - Command list
///     Table     - Status window
///     Paragraph - Command output
fn running<'a>(
    task_list: Arc<Mutex<HashMap<usize, Task>>>,
    cmd_list_state: &ListState,
    scroll: &u16,
) -> (List<'a>, Table<'a>, Paragraph<'a>) {
    let cmd_list: Vec<(usize, String)> = task_list
        .lock()
        .unwrap()
        .iter()
        .map(|task| (*task.0, task.1.command.clone()))
        .collect();
    let items: Vec<_> = cmd_list
        .iter()
        .map(|(_id, cmd)| {
            ListItem::new(Spans::from(vec![Span::styled(
                cmd.clone(),
                Style::default(),
            )]))
        })
        .collect();

    let selected_cmd = if !cmd_list.is_empty() {
        Some(
            cmd_list
                .get(
                    cmd_list_state
                        .selected()
                        .expect("There is always a selected command."),
                )
                .expect("No selected command")
                .clone(),
        )
    } else {
        None
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::White))
                .title("Commands")
                .border_type(BorderType::Plain),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );

    let cmd_stats = Table::new(vec![Row::new(vec![
        Cell::from(Span::raw(match &selected_cmd {
            None => String::new(),
            Some(selected) => selected.1.clone(),
        })),
        Cell::from(Span::raw(match &selected_cmd {
            None => String::new(),
            Some(selected) => {
                let start_time = task_list
                    .lock()
                    .unwrap()
                    .get(&selected.0)
                    .unwrap()
                    .start_time;
                start_time.format("%H:%M:%S").to_string()
            }
        })),
        Cell::from(Span::raw(match &selected_cmd {
            None => String::new(),
            Some(selected) => {
                let start_time = task_list
                    .lock()
                    .unwrap()
                    .get(&selected.0)
                    .unwrap()
                    .start_time;
                match task_list
                    .lock()
                    .unwrap()
                    .get(&selected.0)
                    .unwrap()
                    .finish_time
                {
                    None => "n/a".to_string(),
                    Some(finish_time) => {
                        let execution_time = finish_time.signed_duration_since(start_time);
                        format!(
                            "{}h {}m {}s",
                            execution_time.num_hours(),
                            execution_time.num_minutes(),
                            execution_time.num_seconds()
                        )
                    }
                }
            }
        })),
        Cell::from(Span::raw(match &selected_cmd {
            None => String::new(),
            Some(selected) => task_list
                .lock()
                .unwrap()
                .get(&selected.0)
                .unwrap()
                .status
                .to_string(),
        })),
    ])])
    .header(Row::new(vec![
        Cell::from(Span::styled(
            "Command",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Start Time",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Execution Time",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Status",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Detail")
            .border_type(BorderType::Plain),
    )
    .widths(&[
        Constraint::Percentage(40),
        Constraint::Percentage(30),
        Constraint::Percentage(30),
    ]);

    // The code breaks if refactored as clippy suggested.
    // It has to be this way.
    #[allow(clippy::collapsible_else_if, clippy::unnecessary_unwrap)]
    let exe_res = if selected_cmd.is_none() {
        String::new()
    } else {
        if let Some(output) = &task_list
            .lock()
            .unwrap()
            .get(&selected_cmd.unwrap().0)
            .unwrap()
            .output
        {
            let out = match String::from_utf8_lossy(&output.stdout) {
                std::borrow::Cow::Borrowed(out) => format!("{}\n", out),
                std::borrow::Cow::Owned(_) => String::new(),
            };
            let err = match String::from_utf8_lossy(&output.stderr) {
                std::borrow::Cow::Borrowed(out) => format!("{}\n", out),
                std::borrow::Cow::Owned(_) => String::new(),
            };
            format!("{}\n\n{}", out, err)
        } else {
            String::new()
        }
    };

    let line_count = exe_res.lines().count() as u16;
    let scroll = *min(scroll, &line_count);
    let output_display = Paragraph::new(exe_res)
        .block(
            Block::default()
                .title("Command Output")
                .borders(Borders::ALL),
        )
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .scroll((scroll, 0))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });

    (list, cmd_stats, output_display)
}

#[derive(PartialEq)]
enum InputMode {
    Normal,
    Command,
}

impl fmt::Display for InputMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let res = match *self {
            InputMode::Normal => "Normal",
            InputMode::Command => "Command",
        };
        write!(f, "{res}")
    }
}

struct MapPin(f64, f64);

/// Performance logger, logs performance every `interval`
/// and send the result back for logging.
async fn perf(sender: Sender<PerfData>, interval: Duration) {
    let sys = System::new();
    loop {
        let cpu_usage = match sys.cpu_load_aggregate() {
            Ok(cpu) => {
                sleep(interval);
                let cpu = cpu.done().unwrap();
                ((cpu.user + cpu.system + cpu.nice) * 100f32).round() as u16
            }
            Err(_) => 0,
        };
        let mem_usage = match sys.memory() {
            Ok(mem) => (saturating_sub_bytes(mem.total, mem.free), mem.total),
            Err(_) => (ByteSize(0), ByteSize(0)),
        };
        let _ = sender
            .send(PerfData {
                cpu_usage,
                mem_usage,
            })
            .await;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // database of commands
    let tasks = Tasks::default();
    let mut command_id: usize = 0;

    // start workers
    spawn_threads(10, tasks.clone());

    // Handles user input in a different thread and sends them through a channel.
    let (tx, rx) = channel();
    let tick_rate = Duration::from_millis(200);
    thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));
            if event::poll(timeout).expect("poll works") {
                if let Event::Key(key) = event::read().expect("can read events") {
                    tx.send(UIEvent::Input(key)).expect("can send events");
                }
            }
            if last_tick.elapsed() >= tick_rate && tx.send(UIEvent::Tick).is_ok() {
                last_tick = Instant::now();
            }
        }
    });

    // Add the different pages and select "Running" as the default active one.
    let menu_titles = vec!["Running", "Finished", "Stats", "Map", "Help"];
    let mut active_menu_item = MenuItem::Running;

    // state of the currently running command list in the main page
    let mut running_list_state = ListState::default();
    running_list_state.select(Some(0));

    // state of the finished command list in the main page
    let mut finished_list_state = ListState::default();
    finished_list_state.select(Some(0));
    let mut scroll = 0;

    // Initialize command input prompt
    let mut input_mode = InputMode::Normal;
    let mut command_input = String::from('_');
    let mut command_hist: Vec<String> = vec![];
    load_hist(&mut command_hist);
    let mut curr_hist_index: usize = command_hist.len();

    // Initialize system stats logging
    let log_length = 40;
    let mut cpu_hist: PerfLog<u16> = PerfLog::new(log_length);
    let mut mem_hist: PerfLog<(ByteSize, ByteSize)> = PerfLog::new(log_length);
    let (perf_tx, perf_rx): (Sender<PerfData>, Receiver<PerfData>) = unbounded();
    task::spawn(perf(perf_tx, Duration::from_secs(1)));

    // event loop
    loop {
        // log system stats
        if let Ok(stat) = perf_rx.try_recv() {
            cpu_hist.push(stat.cpu_usage);
            mem_hist.push(stat.mem_usage);
        }

        let currently_running = &tasks.get_currently_running();
        // renders UI
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Length(3), Constraint::Min(7)].as_ref())
                .split(f.size());

            let menu_items = menu_titles
                .iter()
                .map(|t| {
                    let (first, rest) = t.split_at(1);
                    Spans::from(vec![
                        Span::styled(
                            first,
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::UNDERLINED),
                        ),
                        Span::styled(rest, Style::default().fg(Color::White)),
                    ])
                })
                .collect();

            let menu = Tabs::new(menu_items)
                .select(active_menu_item.into())
                .block(Block::default().title("Menu").borders(Borders::ALL))
                .style(Style::default().fg(Color::White))
                .highlight_style(Style::default().fg(Color::Yellow))
                .divider(Span::raw("|"));

            let cli_title = format!("Command Line - Input Mode: {input_mode}");
            let cli = Paragraph::new(command_input.clone())
                .block(Block::default().title(cli_title).borders(Borders::ALL))
                .style(Style::default())
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: true });

            // switch between pages
            match active_menu_item {
                MenuItem::Running => {
                    // three chunks below the tabs chunk
                    let main_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(2), Constraint::Length(5)])
                        .split(chunks[1]);
                    {
                        // List and the two chunks to the right
                        let middle_chunks = Layout::default()
                            .direction(Direction::Horizontal)
                            .constraints(
                                [Constraint::Percentage(20), Constraint::Percentage(80)].as_ref(),
                            )
                            .split(main_chunks[0]);
                        {
                            // Details and Command Output
                            let chunks = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints(
                                    [Constraint::Percentage(20), Constraint::Percentage(80)]
                                        .as_ref(),
                                )
                                .split(middle_chunks[1]);
                            let (cmd_list, stat, output) =
                                running(currently_running.clone(), &running_list_state, &scroll);

                            f.render_stateful_widget(
                                cmd_list,
                                middle_chunks[0],
                                &mut running_list_state,
                            );
                            f.render_widget(stat, chunks[0]);
                            f.render_widget(output, chunks[1]);
                        }
                        f.render_widget(cli, main_chunks[1]);
                    }
                }
                MenuItem::Finished => {
                    // List and the two chunks to the right
                    let middle_chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints(
                            [Constraint::Percentage(20), Constraint::Percentage(80)].as_ref(),
                        )
                        .split(chunks[1]);
                    {
                        // Details and Command Output
                        let chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints(
                                [Constraint::Percentage(20), Constraint::Percentage(80)].as_ref(),
                            )
                            .split(middle_chunks[1]);
                        let (cmd_list, stat, output) =
                            running(tasks.get_finished(), &finished_list_state, &scroll);

                        f.render_stateful_widget(
                            cmd_list,
                            middle_chunks[0],
                            &mut finished_list_state,
                        );
                        f.render_widget(stat, chunks[0]);
                        f.render_widget(output, chunks[1]);
                    }
                }
                MenuItem::Stats => {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Percentage(49), // cpu
                            Constraint::Percentage(2),  // separator
                            Constraint::Percentage(49), // memory
                        ])
                        .split(chunks[1]);

                    // Time axis calculated from `log_length`
                    let mut x_label = vec![];
                    for i in (0..=log_length).rev().step_by(10) {
                        x_label.push(i.to_string());
                    }

                    // CPU
                    let cpu_usage = match cpu_hist.last() {
                        None => 0u16,
                        Some(load) => *load,
                    };
                    let cpu_load = Gauge::default()
                        .block(Block::default().borders(Borders::ALL).title("CPU Usage"))
                        .gauge_style(
                            Style::default()
                                .fg(Color::White)
                                .bg(Color::Black)
                                .add_modifier(Modifier::ITALIC),
                        )
                        .percent(cpu_usage);
                    let cpu_data_points = cpu_hist
                        .iter()
                        .enumerate()
                        .map(|(i, load)| (i as f64, *load as f64))
                        .collect::<Vec<_>>();
                    let cpu_load_hist = vec![Dataset::default()
                        .name("CPU_load")
                        .marker(symbols::Marker::Braille)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(Color::Yellow))
                        .data(cpu_data_points.as_slice())];
                    let cpu_hist_chart = Chart::new(cpu_load_hist)
                        .block(Block::default().title("CPU Usage Chart"))
                        .x_axis(
                            Axis::default()
                                .title(Span::styled("Data Points", Style::default().fg(Color::Red)))
                                .style(Style::default().fg(Color::White))
                                .bounds([0.0, log_length as f64])
                                .labels(x_label.iter().cloned().map(Span::from).collect()),
                        )
                        .y_axis(
                            Axis::default()
                                .title(Span::styled("Load", Style::default().fg(Color::Red)))
                                .style(Style::default().fg(Color::White))
                                .bounds([0.0, 100.0])
                                .labels(
                                    ["0", "50", "100"].iter().cloned().map(Span::from).collect(),
                                ),
                        );

                    // RAM
                    let (used_mem, total_mem) = match mem_hist.last() {
                        None => (ByteSize(0), ByteSize(0)),
                        Some((used, total)) => (*used, *total),
                    };
                    let title = if total_mem == ByteSize(0) {
                        "Error getting memory stats.".to_string()
                    } else {
                        format!("RAM: {used_mem} used of {total_mem}")
                    };
                    let mem_perc = |used: ByteSize, total: ByteSize| {
                        if total.as_u64() != 0 {
                            (used.as_u64() * 100 / total.as_u64()) as u16
                        } else {
                            0
                        }
                    };
                    let mem_usage = Gauge::default()
                        .block(Block::default().borders(Borders::ALL).title(title))
                        .gauge_style(
                            Style::default()
                                .fg(Color::White)
                                .bg(Color::Black)
                                .add_modifier(Modifier::ITALIC),
                        )
                        .percent(mem_perc(used_mem, total_mem));
                    let mem_data_points = mem_hist
                        .iter()
                        .enumerate()
                        .map(|(i, (used, total))| (i as f64, mem_perc(*used, *total) as f64))
                        .collect::<Vec<_>>();
                    let mem_load_hist = vec![Dataset::default()
                        .name("MEM_load")
                        .marker(symbols::Marker::Braille)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(Color::Yellow))
                        .data(mem_data_points.as_slice())];

                    let mem_hist_chart = Chart::new(mem_load_hist)
                        .block(Block::default().title("Memory Usage Chart"))
                        .x_axis(
                            Axis::default()
                                .title(Span::styled("Data Points", Style::default().fg(Color::Red)))
                                .style(Style::default().fg(Color::White))
                                .bounds([0.0, log_length as f64])
                                .labels(x_label.iter().cloned().map(Span::from).collect()),
                        )
                        .y_axis(
                            Axis::default()
                                .title(Span::styled(
                                    "Usage (% Total)",
                                    Style::default().fg(Color::Red),
                                ))
                                .style(Style::default().fg(Color::White))
                                .bounds([0.0, 100.0])
                                .labels(
                                    ["0", "50", "100"].iter().cloned().map(Span::from).collect(),
                                ),
                        );

                    let cpu_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints(
                            [
                                Constraint::Length(3),
                                Constraint::Min(10),
                            ]
                        )
                        .split(chunks[0]);

                    let mem_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints(
                            [
                                Constraint::Length(3),
                                Constraint::Min(10),
                            ]
                        )
                        .split(chunks[2]);

                    f.render_widget(cpu_load, cpu_chunks[0]);
                    f.render_widget(cpu_hist_chart, cpu_chunks[1]);
                    f.render_widget(mem_usage, mem_chunks[0]);
                    f.render_widget(mem_hist_chart, mem_chunks[1]);
                }
                MenuItem::Map => {
                    let servers = vec![
                        MapPin(38.9, 121.2),
                        MapPin(34.2, 108.9),
                        MapPin(23.1, 113.2),
                    ];
                    let map = Canvas::default()
                        .block(Block::default().title("World").borders(Borders::ALL))
                        .paint(|ctx| {
                            ctx.draw(&Map {
                                color: Color::White,
                                resolution: MapResolution::High,
                            });
                            ctx.layer();
                            for (i, s1) in servers.iter().enumerate() {
                                for s2 in &servers[i + 1..] {
                                    ctx.draw(&Line {
                                        x1: s1.1,
                                        y1: s1.0,
                                        y2: s2.0,
                                        x2: s2.1,
                                        color: Color::Yellow,
                                    });
                                }
                            }
                            for server in &servers {
                                let color = Color::Green;
                                ctx.print(
                                    server.1,
                                    server.0,
                                    Span::styled("+", Style::default().fg(color)),
                                );
                            }
                        })
                        .marker(symbols::Marker::Braille)
                        .x_bounds([-180.0, 180.0])
                        .y_bounds([-90.0, 90.0]);
                    f.render_widget(map, chunks[1]);
                }
                MenuItem::Help => {
                    let help_text = "Use the tab's initial to switch to that tab.
                        For example, to switch to 'Stats', press 'S'.

                        While in 'Running' or 'Finished' tab, press 'Up' and 'Down' \
                        to select different entries. Or use 'j' and 'k' like in vim!
                        While in 'Finished' tab, use 'PageUp' and 'PageDown' to scroll \
                        through long outputs.
                        While in 'Running' tab, press 'i' to go into command mode, \
                        this activates the command line input. While in command mode, \
                        use Up and Down to scroll through command history.

                        When you are finished, press 'ESC' to exit insert mode.

                        Command history is persistent, stored in `.cmd_hist`.
                        Press 'q' in Normal mode to exit the program safely.";

                    let help = Paragraph::new(help_text)
                        .block(Block::default().title("Help Message").borders(Borders::ALL))
                        .style(Style::default())
                        .alignment(Alignment::Left)
                        .wrap(Wrap { trim: true });

                    f.render_widget(help, chunks[1]);
                }
            }

            f.render_widget(menu, chunks[0]);
        })?;

        // receives user input from event handler thread
        match rx.recv()? {
            UIEvent::Input(event) => match input_mode {
                InputMode::Normal => match event.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('r') => active_menu_item = MenuItem::Running,
                    KeyCode::Char('f') => active_menu_item = MenuItem::Finished,
                    KeyCode::Char('s') => active_menu_item = MenuItem::Stats,
                    KeyCode::Char('m') => active_menu_item = MenuItem::Map,
                    KeyCode::Char('h') => active_menu_item = MenuItem::Help,
                    KeyCode::PageUp => scroll = if scroll > 2 { scroll - 2 } else { 0 },
                    KeyCode::PageDown => scroll = min(scroll + 2, u16::MAX),
                    KeyCode::Up | KeyCode::Char('k') => {
                        match active_menu_item {
                            MenuItem::Running => {
                                if let Some(selected) = running_list_state.selected() {
                                    if selected > 0 {
                                        running_list_state.select(Some(selected - 1));
                                    }
                                }
                            }
                            MenuItem::Finished => {
                                if let Some(selected) = finished_list_state.selected() {
                                    if selected > 0 {
                                        finished_list_state.select(Some(selected - 1));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => match active_menu_item {
                        MenuItem::Running => {
                            if let Some(selected) = running_list_state.selected() {
                                let num_command = currently_running.lock().unwrap().len();
                                if selected < num_command - 1 {
                                    running_list_state.select(Some(selected + 1));
                                }
                            }
                        }
                        MenuItem::Finished => {
                            if let Some(selected) = finished_list_state.selected() {
                                let num_command = tasks.get_finished().lock().unwrap().len();
                                if selected < num_command - 1 {
                                    finished_list_state.select(Some(selected + 1));
                                }
                            }
                        }
                        _ => {}
                    },
                    KeyCode::Char('i') => {
                        if active_menu_item == MenuItem::Running {
                            input_mode = InputMode::Command;
                        }
                    }
                    _ => {}
                },
                InputMode::Command => match event.code {
                    KeyCode::Char(c) => command_input.insert(command_input.len() - 1, c),
                    KeyCode::Up => {
                        if curr_hist_index > 0 {
                            command_input = command_hist.get(curr_hist_index - 1).unwrap().clone();
                            command_input.push('_');
                            curr_hist_index = max(curr_hist_index - 1, 0);
                        }
                    }
                    KeyCode::Down => {
                        curr_hist_index = min(curr_hist_index + 1, command_hist.len());
                        match curr_hist_index.cmp(&command_hist.len()) {
                            Ordering::Less => {
                                command_input = command_hist.get(curr_hist_index).unwrap().clone();
                                command_input.push('_');
                            }
                            Ordering::Equal => command_input = String::from('_'),
                            Ordering::Greater => {}
                        }
                    }
                    KeyCode::Enter => {
                        command_input.pop();
                        tasks.push_queue(command_id, command_input.clone());
                        command_hist.push(command_input.clone());
                        curr_hist_index = command_hist.len();
                        command_id += 1;
                        command_input = String::from('_');
                    }
                    KeyCode::Backspace => {
                        if command_input.len() > 1 {
                            command_input.remove(command_input.len() - 2);
                        }
                    }
                    KeyCode::Esc => {
                        input_mode = InputMode::Normal;
                    }
                    _ => {}
                },
            },
            UIEvent::Tick => {}
        }
    }

    save_hist(command_hist);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.clear()?;
    terminal.show_cursor()?;

    Ok(())
}

fn load_hist(cmd_hist: &mut Vec<String>) {
    if let Ok(mut file_path) = get_project_root() {
        file_path.push(".cmd_hist");
        if let Ok(f) = File::open(file_path) {
            let reader = BufReader::new(f);
            cmd_hist.extend(reader.lines().map(|line| line.unwrap()));
        }
    }
}

fn save_hist(cmd_hist: Vec<String>) {
    if let Ok(mut file_path) = get_project_root() {
        file_path.push(".cmd_hist");
        if let Ok(mut f) = OpenOptions::new().create(true).write(true).open(file_path) {
            for cmd in cmd_hist {
                let _ = f.write(format!("{cmd}\n").as_ref());
            }
        }
    };
}
