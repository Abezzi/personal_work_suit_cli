use chrono::prelude::*;
use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use rand::{distributions::Alphanumeric, prelude::*};
use serde::{Deserialize, Serialize};
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use std::{collections::HashSet, fs};
use thiserror::Error;
use tui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    symbols::bar::Set,
    text::{Span, Spans},
    widgets::{
        Block, BorderType, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table, Tabs,
    },
    Terminal,
};

const DB_PATH: &str = "./data/db.json";

#[derive(Error, Debug)]
pub enum Error {
    #[error("error reading the DB file: {0}")]
    ReadDBError(#[from] io::Error),
    #[error("error parsing the DB file: {0}")]
    ParseDBError(#[from] serde_json::Error),
}

enum Event<I> {
    Input(I),
    Tick,
}

#[derive(Serialize, Deserialize, Clone)]
struct Timer {
    id: usize,
    name: String,
    category: String,
    created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Todo {
    id: usize,
    title: String,
    description: String,
    category: String,
    status: TodoStatus,
    created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug)]
enum MenuItem {
    Home,
    Todos,
    Timers,
    TimeTracking,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
enum TodoStatus {
    Todo,
    Done,
    Doing,
}

impl From<MenuItem> for usize {
    fn from(input: MenuItem) -> usize {
        match input {
            MenuItem::Home => 0,
            MenuItem::Todos => 1,
            MenuItem::Timers => 2,
            MenuItem::TimeTracking => 3,
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode().expect("can run in raw mode");

    let (tx, rx) = mpsc::channel();
    let tick_rate = Duration::from_millis(200);
    thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if event::poll(timeout).expect("poll works") {
                if let CEvent::Key(key) = event::read().expect("can read events") {
                    tx.send(Event::Input(key)).expect("can send events");
                }
            }

            if last_tick.elapsed() >= tick_rate {
                if let Ok(_) = tx.send(Event::Tick) {
                    last_tick = Instant::now();
                }
            }
        }
    });

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let menu_titles = vec!["Home", "Todos", "Timers", "TimeTracking", "Quit"];
    let mut active_menu_item = MenuItem::Home;
    let mut todo_list_state = ListState::default();
    let mut doing_list_state = ListState::default();
    let mut done_list_state = ListState::default();

    todo_list_state.select(Some(0));
    // doing_list_state.select(None(0));
    // done_list_state.select(Some(0));

    loop {
        terminal.draw(|rect| {
            let size = rect.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints(
                    [
                        Constraint::Length(3),
                        Constraint::Min(2),
                        Constraint::Length(3),
                    ]
                    .as_ref(),
                )
                .split(size);

            let copyright = Paragraph::new("Personal Work Suit CLI - all rights reserved")
                .style(Style::default().fg(Color::LightCyan))
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::White))
                        .title("Copyright")
                        .border_type(BorderType::Plain),
                );

            let mut hotkey_set: HashSet<&str> = HashSet::new();
            let menu = menu_titles
                .iter()
                .map(|t| {
                    let (first, rest) = t.split_at(1);
                    let (second, other_rest) = rest.split_at(1);
                    let (third, other_other_rest) = other_rest.split_at(1);

                    // TODO: ugly code

                    if !hotkey_set.contains(first) {
                        hotkey_set.insert(first);
                        Spans::from(vec![
                            Span::styled(
                                first,
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::UNDERLINED),
                            ),
                            Span::styled(rest, Style::default().fg(Color::White)),
                        ])
                    } else if !hotkey_set.contains(second) {
                        hotkey_set.insert(second);
                        Spans::from(vec![
                            Span::styled(first, Style::default().fg(Color::White)),
                            Span::styled(
                                second,
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::UNDERLINED),
                            ),
                            Span::styled(other_rest, Style::default().fg(Color::White)),
                        ])
                    } else {
                        hotkey_set.insert(third);
                        Spans::from(vec![
                            Span::styled(first, Style::default().fg(Color::White)),
                            Span::styled(second, Style::default().fg(Color::White)),
                            Span::styled(
                                third,
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::UNDERLINED),
                            ),
                            Span::styled(other_other_rest, Style::default().fg(Color::White)),
                        ])
                    }
                })
                .collect();

            let tabs = Tabs::new(menu)
                .select(active_menu_item.into())
                .block(Block::default().title("Menu").borders(Borders::ALL))
                .style(Style::default().fg(Color::White))
                .highlight_style(Style::default().fg(Color::Yellow))
                .divider(Span::raw("|"));

            rect.render_widget(tabs, chunks[0]);
            match active_menu_item {
                MenuItem::Home => rect.render_widget(render_home(), chunks[1]),
                MenuItem::Todos => {
                    let todos_vertical_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints(
                            [Constraint::Percentage(80), Constraint::Percentage(20)].as_ref(),
                        )
                        .split(chunks[1]);

                    let todos_horizontal_chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints(
                            [
                                Constraint::Percentage(33),
                                Constraint::Percentage(33),
                                Constraint::Percentage(33),
                            ]
                            .as_ref(),
                        )
                        .split(todos_vertical_chunks[0]);

                    let (todo_list, doing_list, done_list, details_table) =
                        render_todos(&todo_list_state, &doing_list_state, &done_list_state);

                    // divide thje todo_list_state and use that here
                    rect.render_stateful_widget(
                        todo_list,
                        todos_horizontal_chunks[0],
                        &mut todo_list_state,
                    );

                    rect.render_stateful_widget(
                        doing_list,
                        todos_horizontal_chunks[1],
                        &mut doing_list_state,
                    );

                    rect.render_stateful_widget(
                        done_list,
                        todos_horizontal_chunks[2],
                        &mut done_list_state,
                    );

                    rect.render_widget(details_table, todos_vertical_chunks[1]);
                }
                MenuItem::Timers => {}
                MenuItem::TimeTracking => {}
            }
            rect.render_widget(copyright, chunks[2]);
        })?;

        match rx.recv()? {
            Event::Input(event) => match event.code {
                KeyCode::Char('q') => {
                    disable_raw_mode()?;
                    terminal.show_cursor()?;
                    break;
                }
                KeyCode::Char('w') => active_menu_item = MenuItem::Home,
                KeyCode::Char('t') => active_menu_item = MenuItem::Todos,
                KeyCode::Char('i') => active_menu_item = MenuItem::Timers,
                KeyCode::Char('m') => active_menu_item = MenuItem::TimeTracking,
                // KeyCode::Char('a') => {
                //     add_random_pet_to_db().expect("can add new random pet");
                // }
                // KeyCode::Char('d') => {
                //     remove_pet_at_index(&mut pet_list_state).expect("can remove pet");
                // }
                KeyCode::Left => {
                    //
                }
                KeyCode::Right => {
                    //
                }
                KeyCode::Up => {
                    //
                }
                KeyCode::Down => {
                    //
                }
                KeyCode::Char('h') => {
                    //
                }
                // TODO: move left and right
                KeyCode::Char('j') => {
                    if let Some(selected) = todo_list_state.selected() {
                        let amount_todos = read_db_by_todo_status(TodoStatus::Todo)
                            .expect("can fetch todo list")
                            .len();
                        if selected >= amount_todos - 1 {
                            todo_list_state.select(Some(0));
                        } else {
                            todo_list_state.select(Some(selected + 1));
                        }
                    }
                }
                KeyCode::Char('k') => {
                    if let Some(selected) = todo_list_state.selected() {
                        let amount_todos = read_db_by_todo_status(TodoStatus::Todo)
                            .expect("can fetch todo list")
                            .len();
                        if selected > 0 {
                            todo_list_state.select(Some(selected - 1));
                        } else {
                            todo_list_state.select(Some(amount_todos - 1));
                        }
                    }
                }
                KeyCode::Char('l') => {
                    if let Some(selected) = todo_list_state.selected() {
                        // todo_list_state.
                        // TODO: depsues de cambiar el detail descomentar l ode bajao
                        // todo_list_state.select(None);
                        doing_list_state.select(Some(0));
                        doing_list_state.selected();
                    } else if let Some(selected) = doing_list_state.selected() {
                        done_list_state.select(Some(0));
                        done_list_state.selected();
                    } else if let Some(selected) = done_list_state.selected() {
                    } else {
                    }
                }
                _ => {}
            },
            Event::Tick => {}
        }
    }

    Ok(())
}

fn render_home<'a>() -> Paragraph<'a> {
    let home = Paragraph::new(vec![
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Welcome")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("to")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::styled(
            "Personal Work Suit CLI",
            Style::default().fg(Color::LightBlue),
        )]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw(
            "Press 't' to access To-Do, 'i' to access timers and 'm' to check time tracking.",
        )]),
    ])
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Home")
            .border_type(BorderType::Plain),
    );
    home
}

fn render_todos<'a>(
    todo_list_state: &ListState,
    doing_list_state: &ListState,
    done_list_state: &ListState,
) -> (List<'a>, List<'a>, List<'a>, Table<'a>) {
    let todos_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("ToDo")
        .border_type(BorderType::Plain);

    let doing_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Doing")
        .border_type(BorderType::Plain);

    let done_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Done")
        .border_type(BorderType::Plain);

    // let todo_list = read_db().expect("can fetch todo list");
    let todo_list = read_db_by_todo_status(TodoStatus::Todo).expect("can fetch todo list");
    let doing_list = read_db_by_todo_status(TodoStatus::Doing).expect("can fetch todo list");
    let done_list = read_db_by_todo_status(TodoStatus::Done).expect("can fetch todo list");

    let items_todo: Vec<_> = todo_list
        .iter()
        .map(|todo| {
            ListItem::new(Spans::from(vec![Span::styled(
                todo.title.clone(),
                Style::default(),
            )]))
        })
        .collect();

    let items_doing: Vec<_> = doing_list
        .iter()
        .map(|todo| {
            ListItem::new(Spans::from(vec![Span::styled(
                todo.title.clone(),
                Style::default(),
            )]))
        })
        .collect();

    let items_done: Vec<_> = done_list
        .iter()
        .map(|todo| {
            ListItem::new(Spans::from(vec![Span::styled(
                todo.title.clone(),
                Style::default(),
            )]))
        })
        .collect();

    // TODO: should have only the corresponding column
    let selected_todo = todo_list
        .get(
            todo_list_state
                .selected()
                .expect("there is always a selected todo"),
        )
        .expect("exists")
        .clone();

    let list_todo = List::new(items_todo).block(todos_block).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    let list_doing = List::new(items_doing).block(doing_block).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    let list_done = List::new(items_done).block(done_block).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    let todo_detail = Table::new(vec![Row::new(vec![
        Cell::from(Span::raw(selected_todo.id.to_string())),
        Cell::from(Span::raw(selected_todo.title)),
        Cell::from(Span::raw(selected_todo.description)),
        Cell::from(Span::raw(selected_todo.category)),
        Cell::from(Span::raw(selected_todo.created_at.to_string())),
    ])])
    .header(Row::new(vec![
        Cell::from(Span::styled(
            "ID",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Title",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Description",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Category",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Created At",
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
        Constraint::Percentage(5),  // id
        Constraint::Percentage(20), // title
        Constraint::Percentage(20), // description
        Constraint::Percentage(20), // category
        Constraint::Percentage(20), // date
    ]);

    (list_todo, list_doing, list_done, todo_detail)
}

fn read_db() -> Result<Vec<Todo>, Error> {
    let db_content = fs::read_to_string(DB_PATH)?;
    let parsed: Vec<Todo> = serde_json::from_str(&db_content)?;
    Ok(parsed)
}

fn read_db_by_todo_status(status: TodoStatus) -> Result<Vec<Todo>, Error> {
    let db_content = fs::read_to_string(DB_PATH)?;
    let parsed: Vec<Todo> = serde_json::from_str(&db_content)?;
    let filtered: Vec<Todo> = parsed
        .iter()
        .filter(|s| s.status == status)
        .cloned()
        .collect();

    // for p in filtered.iter() {
    //     println!("{}", p.description);
    // }

    Ok(filtered)
}
