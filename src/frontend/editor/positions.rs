//! Small UTF-16 checkpoints keep bulk editor locations proportional to source size.
use std::collections::TryReserveError;

const STRIDE: usize = 256;

#[derive(Clone, Copy)]
struct Point {
    byte: usize,
    line: usize,
    column: usize,
    carriage: bool,
}

impl Point {
    fn advance(&mut self, character: char) {
        match character {
            '\r' => {
                self.line += 1;
                self.column = 0;
                self.carriage = true;
            }
            '\n' => {
                if !self.carriage {
                    self.line += 1;
                }
                self.column = 0;
                self.carriage = false;
            }
            _ => {
                self.column += character.len_utf16();
                self.carriage = false;
            }
        }
    }
}

pub(super) struct Positions<'a> {
    text: &'a str,
    points: Vec<Point>,
}

impl<'a> Positions<'a> {
    pub fn new(text: &'a str) -> Result<Self, TryReserveError> {
        let mut points = Vec::new();
        points.try_reserve_exact(text.len() / STRIDE + 1)?;
        let mut current = Point {
            byte: 0,
            line: 0,
            column: 0,
            carriage: false,
        };
        points.push(current);
        let mut checkpoint = 0;
        for (byte, character) in text.char_indices() {
            if byte - checkpoint >= STRIDE {
                current.byte = byte;
                points.push(current);
                checkpoint = byte;
            }
            current.advance(character);
        }
        Ok(Self { text, points })
    }

    pub fn at(&self, offset: usize) -> (usize, usize) {
        assert!(offset <= self.text.len() && self.text.is_char_boundary(offset));
        let offset = if self.text.as_bytes().get(offset) == Some(&b'\n')
            && offset > 0
            && self.text.as_bytes()[offset - 1] == b'\r'
        {
            offset - 1
        } else {
            offset
        };
        let index = self.points.partition_point(|point| point.byte <= offset) - 1;
        let mut point = self.points[index];
        for character in self.text[point.byte..offset].chars() {
            point.advance(character);
        }
        (point.line, point.column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{project::budget::Allocator, source::SourceMap};

    #[test]
    fn every_unicode_boundary_matches_the_existing_editor_coordinate_contract() {
        for text in [
            String::new(),
            "a🦀b\r\n雪\rc\n".repeat(128),
            format!("{}🦀\r\n{}\r雪", "x".repeat(255), "z".repeat(512)),
        ] {
            let mut sources = SourceMap::new();
            let id = sources
                .try_add(
                    "positions.ox".into(),
                    text.clone(),
                    &mut Allocator::default(),
                )
                .unwrap();
            let source = sources.get(id);
            let positions = Positions::new(&text).unwrap();
            for offset in text
                .char_indices()
                .map(|(offset, _)| offset)
                .chain(std::iter::once(text.len()))
            {
                assert_eq!(
                    positions.at(offset),
                    source.editor_position(offset),
                    "offset {offset}"
                );
            }
        }
    }

    #[test]
    fn queries_scan_only_one_bounded_utf8_checkpoint_suffix() {
        let text = format!("{}🦀\r\n雪\r", "x".repeat(255)).repeat(64);
        let positions = Positions::new(&text).unwrap();
        assert!(positions.points.len() <= text.len() / STRIDE + 1);
        for offset in text
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(text.len()))
        {
            let index = positions
                .points
                .partition_point(|point| point.byte <= offset)
                - 1;
            assert!(offset - positions.points[index].byte < STRIDE + 4);
        }
    }
}
