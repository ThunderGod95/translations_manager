use std::io;

use anyhow::Result;
use prettytable::{Cell, Row, Table, format};
use terminal_size::{Width, terminal_size};

use super::single::Match;

#[derive(Debug, Clone, Copy)]
pub enum FormatOption {
    Paragraphs,
    Table,
}

pub fn format_folder_matches<W: io::Write>(
    matches: &Vec<(usize, Vec<Match>)>,
    format_as: FormatOption,
    mut writer: W,
) -> Result<()> {
    match format_as {
        FormatOption::Table => write_table(matches, &mut writer),
        FormatOption::Paragraphs => write_paragraphs(matches, &mut writer),
    }
}

fn write_table<W: io::Write>(matches: &Vec<(usize, Vec<Match>)>, writer: &mut W) -> Result<()> {
    if matches.is_empty() {
        return Ok(());
    }

    let terminal_width = terminal_size()
        .map(|(Width(w), _)| w as usize)
        .unwrap_or(80);

    const FILE_HEADER: &str = "File";
    const LINE_HEADER: &str = "Line No.";
    const TEXT_HEADER: &str = "Text";

    // Assumption: File/line numbers won't exceed 6 chars
    const MIN_NUM_WIDTH: usize = 6;
    // | Col1 | Col2 | Col3 | -> 4 borders
    // 3 columns * 2 spaces/col padding = 6 padding
    const TABLE_FORMATTING_OVERHEAD: usize = 10;

    // Calculate column widths based on headers or assumed number width
    let file_col_width = std::cmp::max(FILE_HEADER.len(), MIN_NUM_WIDTH);
    let line_col_width = std::cmp::max(LINE_HEADER.len(), MIN_NUM_WIDTH);

    let total_overhead = file_col_width + line_col_width + TABLE_FORMATTING_OVERHEAD;
    let max_text_width = terminal_width.saturating_sub(total_overhead);

    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_DEFAULT);
    table.add_row(Row::from(vec![FILE_HEADER, LINE_HEADER, TEXT_HEADER]));

    for (file_num, matches_in_file) in matches {
        for m in matches_in_file {
            let wrapped_content = textwrap::fill(m.line_content.trim(), max_text_width);

            table.add_row(Row::new(vec![
                Cell::new(&file_num.to_string()),
                Cell::new(&m.line_number.to_string()),
                Cell::new(&wrapped_content),
            ]));
        }
    }

    write!(writer, "{}", table)?;
    Ok(())
}

fn write_paragraphs<W: io::Write>(
    matches: &Vec<(usize, Vec<Match>)>,
    writer: &mut W,
) -> Result<()> {
    let mut matches_iter = matches.iter().peekable();

    while let Some((file_num, file_matches)) = matches_iter.next() {
        writeln!(writer, "# Chapter {}", file_num)?;
        writeln!(writer)?; // Add a single blank line

        for mat in file_matches {
            writeln!(writer, "[{}] {}", mat.line_number, mat.line_content.trim())?;
        }

        if matches_iter.peek().is_some() {
            write!(writer, "\n---\n\n")?;
        }
    }

    Ok(())
}
