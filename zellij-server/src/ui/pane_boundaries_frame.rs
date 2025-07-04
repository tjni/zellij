use crate::output::CharacterChunk;
use crate::panes::{AnsiCode, RcCharacterStyles, TerminalCharacter, EMPTY_TERMINAL_CHARACTER};
use crate::ui::boundaries::boundary_type;
use crate::ClientId;
use zellij_utils::data::{client_id_to_colors, PaletteColor, Style};
use zellij_utils::errors::prelude::*;
use zellij_utils::pane_size::{Offset, Viewport};
use zellij_utils::position::Position;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

fn foreground_color(characters: &str, color: Option<PaletteColor>) -> Vec<TerminalCharacter> {
    let mut colored_string = Vec::new();
    for character in characters.chars() {
        let mut styles = RcCharacterStyles::reset();
        styles.update(|styles| {
            styles.bold = Some(AnsiCode::On);
            match color {
                Some(palette_color) => {
                    styles.foreground = Some(AnsiCode::from(palette_color));
                },
                None => {},
            }
        });
        let terminal_character = TerminalCharacter::new_styled(character, styles);
        colored_string.push(terminal_character);
    }
    colored_string
}

fn background_color(characters: &str, color: Option<PaletteColor>) -> Vec<TerminalCharacter> {
    let mut colored_string = Vec::new();
    for character in characters.chars() {
        let mut styles = RcCharacterStyles::reset();
        styles.update(|styles| match color {
            Some(palette_color) => {
                styles.background = Some(AnsiCode::from(palette_color));
                styles.bold(Some(AnsiCode::On));
            },
            None => {},
        });
        let terminal_character = TerminalCharacter::new_styled(character, styles);
        colored_string.push(terminal_character);
    }
    colored_string
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ExitStatus {
    Code(i32),
    Exited,
}

pub struct FrameParams {
    pub focused_client: Option<ClientId>,
    pub is_main_client: bool, // more accurately: is_focused_for_main_client
    pub other_focused_clients: Vec<ClientId>,
    pub style: Style,
    pub color: Option<PaletteColor>,
    pub other_cursors_exist_in_session: bool,
    pub pane_is_stacked_under: bool,
    pub pane_is_stacked_over: bool,
    pub should_draw_pane_frames: bool,
    pub pane_is_floating: bool,
    pub content_offset: Offset,
    pub mouse_is_hovering_over_pane: bool,
    pub pane_is_selectable: bool,
}

#[derive(Default, PartialEq)]
pub struct PaneFrame {
    pub geom: Viewport,
    pub title: String,
    pub scroll_position: (usize, usize), // (position, length)
    pub style: Style,
    pub color: Option<PaletteColor>,
    pub focused_client: Option<ClientId>,
    pub is_main_client: bool,
    pub other_cursors_exist_in_session: bool,
    pub other_focused_clients: Vec<ClientId>,
    exit_status: Option<ExitStatus>,
    is_first_run: bool,
    pane_is_stacked_over: bool,
    pane_is_stacked_under: bool,
    should_draw_pane_frames: bool,
    is_pinned: bool,
    is_floating: bool,
    content_offset: Offset,
    mouse_is_hovering_over_pane: bool,
    is_selectable: bool,
}

impl PaneFrame {
    pub fn new(
        geom: Viewport,
        scroll_position: (usize, usize),
        main_title: String,
        frame_params: FrameParams,
    ) -> Self {
        PaneFrame {
            geom,
            title: main_title,
            scroll_position,
            style: frame_params.style,
            color: frame_params.color,
            focused_client: frame_params.focused_client,
            is_main_client: frame_params.is_main_client,
            other_focused_clients: frame_params.other_focused_clients,
            other_cursors_exist_in_session: frame_params.other_cursors_exist_in_session,
            exit_status: None,
            is_first_run: false,
            pane_is_stacked_over: frame_params.pane_is_stacked_over,
            pane_is_stacked_under: frame_params.pane_is_stacked_under,
            should_draw_pane_frames: frame_params.should_draw_pane_frames,
            is_pinned: false,
            is_floating: frame_params.pane_is_floating,
            content_offset: frame_params.content_offset,
            mouse_is_hovering_over_pane: frame_params.mouse_is_hovering_over_pane,
            is_selectable: frame_params.pane_is_selectable,
        }
    }
    pub fn is_pinned(mut self, is_pinned: bool) -> Self {
        self.is_pinned = is_pinned;
        self
    }
    pub fn add_exit_status(&mut self, exit_status: Option<i32>) {
        self.exit_status = match exit_status {
            Some(exit_status) => Some(ExitStatus::Code(exit_status)),
            None => Some(ExitStatus::Exited),
        };
    }
    pub fn indicate_first_run(&mut self) {
        self.is_first_run = true;
    }
    pub fn override_color(&mut self, color: PaletteColor) {
        self.color = Some(color);
    }
    fn client_cursor(&self, client_id: ClientId) -> Vec<TerminalCharacter> {
        let color = client_id_to_colors(client_id, self.style.colors.multiplayer_user_colors);
        background_color(" ", color.map(|c| c.0))
    }
    fn get_corner(&self, corner: &'static str) -> &'static str {
        let corner = if !self.should_draw_pane_frames
            && (corner == boundary_type::TOP_LEFT || corner == boundary_type::TOP_RIGHT)
        {
            boundary_type::HORIZONTAL
        } else if self.pane_is_stacked_under && corner == boundary_type::TOP_RIGHT {
            boundary_type::BOTTOM_RIGHT
        } else if self.pane_is_stacked_under && corner == boundary_type::TOP_LEFT {
            boundary_type::BOTTOM_LEFT
        } else {
            corner
        };
        if self.style.rounded_corners {
            match corner {
                boundary_type::TOP_RIGHT => boundary_type::TOP_RIGHT_ROUND,
                boundary_type::TOP_LEFT => boundary_type::TOP_LEFT_ROUND,
                boundary_type::BOTTOM_RIGHT => boundary_type::BOTTOM_RIGHT_ROUND,
                boundary_type::BOTTOM_LEFT => boundary_type::BOTTOM_LEFT_ROUND,
                _ => corner,
            }
        } else {
            corner
        }
    }
    fn render_title_right_side(
        &self,
        max_length: usize,
    ) -> Option<(Vec<TerminalCharacter>, usize)> {
        // string and length because of color
        let has_scroll = self.scroll_position.0 > 0 || self.scroll_position.1 > 0;
        if has_scroll && self.is_selectable {
            // TODO: don't show SCROLL at all for plugins
            let pin_indication = if self.is_floating && self.is_selectable {
                self.render_pinned_indication(max_length)
            } else {
                None
            }; // no pin indication for tiled panes
            let space_for_scroll_indication = pin_indication
                .as_ref()
                .map(|(_, length)| max_length.saturating_sub(*length + 1))
                .unwrap_or(max_length);
            let scroll_indication = self.render_scroll_indication(space_for_scroll_indication);
            match (pin_indication, scroll_indication) {
                (
                    Some((mut pin_indication, pin_indication_len)),
                    Some((mut scroll_indication, scroll_indication_len)),
                ) => {
                    let mut characters: Vec<_> = scroll_indication.drain(..).collect();
                    let mut separator = foreground_color(&format!("|"), self.color);
                    characters.append(&mut separator);
                    characters.append(&mut pin_indication);
                    Some((characters, pin_indication_len + scroll_indication_len + 1))
                },
                (Some(pin_indication), None) => Some(pin_indication),
                (None, Some(scroll_indication)) => Some(scroll_indication),
                _ => None,
            }
        } else if self.is_floating && self.is_selectable {
            self.render_pinned_indication(max_length)
        } else {
            None
        }
    }
    fn render_scroll_indication(
        &self,
        max_length: usize,
    ) -> Option<(Vec<TerminalCharacter>, usize)> {
        let prefix = " SCROLL: ";
        let full_indication = format!(" {}/{} ", self.scroll_position.0, self.scroll_position.1);
        let short_indication = format!(" {} ", self.scroll_position.0);
        let full_indication_len = full_indication.chars().count();
        let short_indication_len = short_indication.chars().count();
        let prefix_len = prefix.chars().count();
        if prefix_len + full_indication_len <= max_length {
            Some((
                foreground_color(&format!("{}{}", prefix, full_indication), self.color),
                prefix_len + full_indication_len,
            ))
        } else if full_indication_len <= max_length {
            Some((
                foreground_color(&full_indication, self.color),
                full_indication_len,
            ))
        } else if short_indication_len <= max_length {
            Some((
                foreground_color(&short_indication, self.color),
                short_indication_len,
            ))
        } else {
            None
        }
    }
    fn render_pinned_indication(
        &self,
        max_length: usize,
    ) -> Option<(Vec<TerminalCharacter>, usize)> {
        let is_checked = if self.is_pinned { '+' } else { ' ' };
        let full_indication = format!(" PIN [{}] ", is_checked);
        let full_indication_len = full_indication.chars().count();
        if full_indication_len <= max_length {
            Some((
                foreground_color(&full_indication, self.color),
                full_indication_len,
            ))
        } else {
            None
        }
    }
    fn render_my_focus(&self, max_length: usize) -> Option<(Vec<TerminalCharacter>, usize)> {
        let mut left_separator = foreground_color(boundary_type::VERTICAL_LEFT, self.color);
        let mut right_separator = foreground_color(boundary_type::VERTICAL_RIGHT, self.color);
        let full_indication_text = "MY FOCUS";
        let mut full_indication = vec![];
        full_indication.append(&mut left_separator);
        full_indication.push(EMPTY_TERMINAL_CHARACTER);
        full_indication.append(&mut foreground_color(full_indication_text, self.color));
        full_indication.push(EMPTY_TERMINAL_CHARACTER);
        full_indication.append(&mut right_separator);
        let full_indication_len = full_indication_text.width() + 4; // 2 for separators 2 for padding
        let short_indication_text = "ME";
        let mut short_indication = vec![];
        short_indication.append(&mut left_separator);
        short_indication.push(EMPTY_TERMINAL_CHARACTER);
        short_indication.append(&mut foreground_color(short_indication_text, self.color));
        short_indication.push(EMPTY_TERMINAL_CHARACTER);
        short_indication.append(&mut right_separator);
        let short_indication_len = short_indication_text.width() + 4; // 2 for separators 2 for padding
        if full_indication_len <= max_length {
            Some((full_indication, full_indication_len))
        } else if short_indication_len <= max_length {
            Some((short_indication, short_indication_len))
        } else {
            None
        }
    }
    fn render_my_and_others_focus(
        &self,
        max_length: usize,
    ) -> Option<(Vec<TerminalCharacter>, usize)> {
        let mut left_separator = foreground_color(boundary_type::VERTICAL_LEFT, self.color);
        let mut right_separator = foreground_color(boundary_type::VERTICAL_RIGHT, self.color);
        let full_indication_text = "MY FOCUS AND:";
        let short_indication_text = "+";
        let mut full_indication = foreground_color(full_indication_text, self.color);
        let mut full_indication_len = full_indication_text.width();
        let mut short_indication = foreground_color(short_indication_text, self.color);
        let mut short_indication_len = short_indication_text.width();
        for client_id in &self.other_focused_clients {
            let mut text = self.client_cursor(*client_id);
            full_indication_len += 2;
            full_indication.push(EMPTY_TERMINAL_CHARACTER);
            full_indication.append(&mut text.clone());
            short_indication_len += 2;
            short_indication.push(EMPTY_TERMINAL_CHARACTER);
            short_indication.append(&mut text);
        }
        if full_indication_len + 4 <= max_length {
            // 2 for separators, 2 for padding
            let mut ret = vec![];
            ret.append(&mut left_separator);
            ret.push(EMPTY_TERMINAL_CHARACTER);
            ret.append(&mut full_indication);
            ret.push(EMPTY_TERMINAL_CHARACTER);
            ret.append(&mut right_separator);
            Some((ret, full_indication_len + 4))
        } else if short_indication_len + 4 <= max_length {
            // 2 for separators, 2 for padding
            let mut ret = vec![];
            ret.append(&mut left_separator);
            ret.push(EMPTY_TERMINAL_CHARACTER);
            ret.append(&mut short_indication);
            ret.push(EMPTY_TERMINAL_CHARACTER);
            ret.append(&mut right_separator);
            Some((ret, short_indication_len + 4))
        } else {
            None
        }
    }
    fn render_other_focused_users(
        &self,
        max_length: usize,
    ) -> Option<(Vec<TerminalCharacter>, usize)> {
        let mut left_separator = foreground_color(boundary_type::VERTICAL_LEFT, self.color);
        let mut right_separator = foreground_color(boundary_type::VERTICAL_RIGHT, self.color);
        let full_indication_text = if self.other_focused_clients.len() == 1 {
            "FOCUSED USER:"
        } else {
            "FOCUSED USERS:"
        };
        let middle_indication_text = "U:";
        let mut full_indication = foreground_color(full_indication_text, self.color);
        let mut full_indication_len = full_indication_text.width();
        let mut middle_indication = foreground_color(middle_indication_text, self.color);
        let mut middle_indication_len = middle_indication_text.width();
        let mut short_indication = vec![];
        let mut short_indication_len = 0;
        for client_id in &self.other_focused_clients {
            let mut text = self.client_cursor(*client_id);
            full_indication_len += 2;
            full_indication.push(EMPTY_TERMINAL_CHARACTER);
            full_indication.append(&mut text.clone());
            middle_indication_len += 2;
            middle_indication.push(EMPTY_TERMINAL_CHARACTER);
            middle_indication.append(&mut text.clone());
            short_indication_len += 2;
            short_indication.push(EMPTY_TERMINAL_CHARACTER);
            short_indication.append(&mut text);
        }
        if full_indication_len + 4 <= max_length {
            // 2 for separators, 2 for padding
            let mut ret = vec![];
            ret.append(&mut left_separator);
            ret.push(EMPTY_TERMINAL_CHARACTER);
            ret.append(&mut full_indication);
            ret.push(EMPTY_TERMINAL_CHARACTER);
            ret.append(&mut right_separator);
            Some((ret, full_indication_len + 4))
        } else if middle_indication_len + 4 <= max_length {
            // 2 for separators, 2 for padding
            let mut ret = vec![];
            ret.append(&mut left_separator);
            ret.push(EMPTY_TERMINAL_CHARACTER);
            ret.append(&mut middle_indication);
            ret.push(EMPTY_TERMINAL_CHARACTER);
            ret.append(&mut right_separator);
            Some((ret, middle_indication_len + 4))
        } else if short_indication_len + 3 <= max_length {
            // 2 for separators, 1 for padding
            let mut ret = vec![];
            ret.append(&mut left_separator);
            ret.push(EMPTY_TERMINAL_CHARACTER);
            ret.append(&mut short_indication);
            ret.push(EMPTY_TERMINAL_CHARACTER);
            ret.append(&mut right_separator);
            Some((ret, short_indication_len + 3))
        } else {
            None
        }
    }
    fn render_title_middle(&self, max_length: usize) -> Option<(Vec<TerminalCharacter>, usize)> {
        // string and length because of color
        if self.is_main_client
            && self.other_focused_clients.is_empty()
            && !self.other_cursors_exist_in_session
        {
            None
        } else if self.is_main_client
            && self.other_focused_clients.is_empty()
            && self.other_cursors_exist_in_session
        {
            self.render_my_focus(max_length)
        } else if self.is_main_client && !self.other_focused_clients.is_empty() {
            self.render_my_and_others_focus(max_length)
        } else if !self.other_focused_clients.is_empty() {
            self.render_other_focused_users(max_length)
        } else if (self.pane_is_stacked_under || self.pane_is_stacked_over)
            && self.exit_status.is_some()
        {
            let (first_part, first_part_len) = self.first_exited_held_title_part_full();
            if first_part_len <= max_length {
                Some((first_part, first_part_len))
            } else {
                None
            }
        } else {
            None
        }
    }
    fn render_title_left_side(&self, max_length: usize) -> Option<(Vec<TerminalCharacter>, usize)> {
        let middle_truncated_sign = "[..]";
        let middle_truncated_sign_long = "[...]";
        let full_text = format!(" {} ", &self.title);
        if max_length <= 6 || self.title.is_empty() {
            None
        } else if full_text.width() <= max_length {
            Some((foreground_color(&full_text, self.color), full_text.width()))
        } else {
            let length_of_each_half = (max_length - middle_truncated_sign.width()) / 2;

            let mut first_part: String = String::new();
            for char in full_text.chars() {
                if first_part.width() + char.width().unwrap_or(0) > length_of_each_half {
                    break;
                } else {
                    first_part.push(char);
                }
            }

            let mut second_part: String = String::new();
            for char in full_text.chars().rev() {
                if second_part.width() + char.width().unwrap_or(0) > length_of_each_half {
                    break;
                } else {
                    second_part.insert(0, char);
                }
            }

            let (title_left_side, title_length) = if first_part.width()
                + middle_truncated_sign.width()
                + second_part.width()
                < max_length
            {
                // this means we lost 1 character when dividing the total length into halves
                (
                    format!(
                        "{}{}{}",
                        first_part, middle_truncated_sign_long, second_part
                    ),
                    first_part.width() + middle_truncated_sign_long.width() + second_part.width(),
                )
            } else {
                (
                    format!("{}{}{}", first_part, middle_truncated_sign, second_part),
                    first_part.width() + middle_truncated_sign.width() + second_part.width(),
                )
            };
            Some((foreground_color(&title_left_side, self.color), title_length))
        }
    }
    fn three_part_title_line(
        &self,
        mut left_side: Vec<TerminalCharacter>,
        left_side_len: &usize,
        mut middle: Vec<TerminalCharacter>,
        middle_len: &usize,
        mut right_side: Vec<TerminalCharacter>,
        right_side_len: &usize,
    ) -> Vec<TerminalCharacter> {
        let total_title_length = self.geom.cols.saturating_sub(2); // 2 for the left and right corners
        let mut title_line = vec![];
        let left_side_start_position = self.geom.x + 1;
        let middle_start_position = self.geom.x + (total_title_length / 2) - (middle_len / 2) + 1;
        let right_side_start_position =
            (self.geom.x + self.geom.cols - 1).saturating_sub(*right_side_len);

        let mut col = self.geom.x;
        loop {
            if col == self.geom.x {
                title_line.append(&mut foreground_color(
                    self.get_corner(boundary_type::TOP_LEFT),
                    self.color,
                ));
            } else if col == self.geom.x + self.geom.cols - 1 {
                title_line.append(&mut foreground_color(
                    self.get_corner(boundary_type::TOP_RIGHT),
                    self.color,
                ));
            } else if col == left_side_start_position {
                title_line.append(&mut left_side);
                col += left_side_len;
                continue;
            } else if col == middle_start_position {
                title_line.append(&mut middle);
                col += middle_len;
                continue;
            } else if col == right_side_start_position {
                title_line.append(&mut right_side);
                col += right_side_len;
                continue;
            } else {
                title_line.append(&mut foreground_color(boundary_type::HORIZONTAL, self.color));
            }
            if col == self.geom.x + self.geom.cols - 1 {
                break;
            }
            col += 1;
        }
        title_line
    }
    fn left_and_middle_title_line(
        &self,
        mut left_side: Vec<TerminalCharacter>,
        left_side_len: &usize,
        mut middle: Vec<TerminalCharacter>,
        middle_len: &usize,
    ) -> Vec<TerminalCharacter> {
        let total_title_length = self.geom.cols.saturating_sub(2); // 2 for the left and right corners
        let mut title_line = vec![];
        let left_side_start_position = self.geom.x + 1;
        let middle_start_position = self.geom.x + (total_title_length / 2) - (*middle_len / 2) + 1;

        let mut col = self.geom.x;
        loop {
            if col == self.geom.x {
                title_line.append(&mut foreground_color(
                    self.get_corner(boundary_type::TOP_LEFT),
                    self.color,
                ));
            } else if col == self.geom.x + self.geom.cols - 1 {
                title_line.append(&mut foreground_color(
                    self.get_corner(boundary_type::TOP_RIGHT),
                    self.color,
                ));
            } else if col == left_side_start_position {
                title_line.append(&mut left_side);
                col += *left_side_len;
                continue;
            } else if col == middle_start_position {
                title_line.append(&mut middle);
                col += *middle_len;
                continue;
            } else {
                title_line.append(&mut foreground_color(boundary_type::HORIZONTAL, self.color));
            }
            if col == self.geom.x + self.geom.cols - 1 {
                break;
            }
            col += 1;
        }
        title_line
    }
    fn middle_only_title_line(
        &self,
        mut middle: Vec<TerminalCharacter>,
        middle_len: &usize,
    ) -> Vec<TerminalCharacter> {
        let total_title_length = self.geom.cols.saturating_sub(2); // 2 for the left and right corners
        let mut title_line = vec![];
        let middle_start_position = self.geom.x + (total_title_length / 2) - (*middle_len / 2) + 1;

        let mut col = self.geom.x;
        loop {
            if col == self.geom.x {
                title_line.append(&mut foreground_color(
                    self.get_corner(boundary_type::TOP_LEFT),
                    self.color,
                ));
            } else if col == self.geom.x + self.geom.cols - 1 {
                title_line.append(&mut foreground_color(
                    self.get_corner(boundary_type::TOP_RIGHT),
                    self.color,
                ));
            } else if col == middle_start_position {
                title_line.append(&mut middle);
                col += *middle_len;
                continue;
            } else {
                title_line.append(&mut foreground_color(boundary_type::HORIZONTAL, self.color));
            }
            if col == self.geom.x + self.geom.cols - 1 {
                break;
            }
            col += 1;
        }
        title_line
    }
    fn two_part_title_line(
        &self,
        mut left_side: Vec<TerminalCharacter>,
        left_side_len: &usize,
        mut right_side: Vec<TerminalCharacter>,
        right_side_len: &usize,
    ) -> Vec<TerminalCharacter> {
        let mut left_boundary =
            foreground_color(self.get_corner(boundary_type::TOP_LEFT), self.color);
        let mut right_boundary =
            foreground_color(self.get_corner(boundary_type::TOP_RIGHT), self.color);
        let total_title_length = self.geom.cols.saturating_sub(2); // 2 for the left and right corners
        let mut middle = String::new();
        for _ in (left_side_len + right_side_len)..total_title_length {
            middle.push_str(boundary_type::HORIZONTAL);
        }
        let mut ret = vec![];
        ret.append(&mut left_boundary);
        ret.append(&mut left_side);
        ret.append(&mut foreground_color(&middle, self.color));
        ret.append(&mut right_side);
        ret.append(&mut right_boundary);
        ret
    }
    fn left_only_title_line(
        &self,
        mut left_side: Vec<TerminalCharacter>,
        left_side_len: &usize,
    ) -> Vec<TerminalCharacter> {
        let mut left_boundary =
            foreground_color(self.get_corner(boundary_type::TOP_LEFT), self.color);
        let mut right_boundary =
            foreground_color(self.get_corner(boundary_type::TOP_RIGHT), self.color);
        let total_title_length = self.geom.cols.saturating_sub(2); // 2 for the left and right corners
        let mut middle_padding = String::new();
        for _ in *left_side_len..total_title_length {
            middle_padding.push_str(boundary_type::HORIZONTAL);
        }
        let mut ret = vec![];
        ret.append(&mut left_boundary);
        ret.append(&mut left_side);
        ret.append(&mut foreground_color(&middle_padding, self.color));
        ret.append(&mut right_boundary);
        ret
    }
    fn empty_title_line(&self) -> Vec<TerminalCharacter> {
        let mut left_boundary =
            foreground_color(self.get_corner(boundary_type::TOP_LEFT), self.color);
        let mut right_boundary =
            foreground_color(self.get_corner(boundary_type::TOP_RIGHT), self.color);
        let total_title_length = self.geom.cols.saturating_sub(2); // 2 for the left and right corners
        let mut middle_padding = String::new();
        for _ in 0..total_title_length {
            middle_padding.push_str(boundary_type::HORIZONTAL);
        }
        let mut ret = vec![];
        ret.append(&mut left_boundary);
        ret.append(&mut foreground_color(&middle_padding, self.color));
        ret.append(&mut right_boundary);
        ret
    }
    fn title_line_with_middle(
        &self,
        middle: Vec<TerminalCharacter>,
        middle_len: &usize,
    ) -> Vec<TerminalCharacter> {
        let total_title_length = self.geom.cols.saturating_sub(2); // 2 for the left and right corners
        let length_of_each_side = total_title_length.saturating_sub(*middle_len + 2) / 2;
        let left_side = self.render_title_left_side(length_of_each_side);
        let right_side = self.render_title_right_side(length_of_each_side);

        match (left_side, right_side) {
            (Some((left_side, left_side_len)), Some((right_side, right_side_len))) => self
                .three_part_title_line(
                    left_side,
                    &left_side_len,
                    middle,
                    middle_len,
                    right_side,
                    &right_side_len,
                ),
            (Some((left_side, left_side_len)), None) => {
                self.left_and_middle_title_line(left_side, &left_side_len, middle, middle_len)
            },
            _ => self.middle_only_title_line(middle, middle_len),
        }
    }
    fn title_line_without_middle(&self) -> Vec<TerminalCharacter> {
        let total_title_length = self.geom.cols.saturating_sub(2); // 2 for the left and right corners
        let left_side = self.render_title_left_side(total_title_length);
        let right_side = left_side.as_ref().and_then(|(_left_side, left_side_len)| {
            let space_left = total_title_length.saturating_sub(*left_side_len + 1); // 1 for a middle separator
            self.render_title_right_side(space_left)
        });
        match (left_side, right_side) {
            (Some((left_side, left_side_len)), Some((right_side, right_side_len))) => {
                self.two_part_title_line(left_side, &left_side_len, right_side, &right_side_len)
            },
            (Some((left_side, left_side_len)), None) => {
                self.left_only_title_line(left_side, &left_side_len)
            },
            _ => self.empty_title_line(),
        }
    }
    fn render_title(&self) -> Result<Vec<TerminalCharacter>> {
        let total_title_length = self.geom.cols.saturating_sub(2); // 2 for the left and right corners

        self.render_title_middle(total_title_length)
            .map(|(middle, middle_length)| self.title_line_with_middle(middle, &middle_length))
            .or_else(|| Some(self.title_line_without_middle()))
            .with_context(|| format!("failed to render title '{}'", self.title))
    }
    fn render_one_line_title(&self) -> Result<Vec<TerminalCharacter>> {
        let total_title_length = self.geom.cols.saturating_sub(2); // 2 for the left and right corners

        self.render_title_middle(total_title_length)
            .map(|(middle, middle_length)| self.title_line_with_middle(middle, &middle_length))
            .or_else(|| Some(self.title_line_without_middle()))
            .with_context(|| format!("failed to render title '{}'", self.title))
    }
    fn render_held_undertitle(&self) -> Result<Vec<TerminalCharacter>> {
        let max_undertitle_length = self.geom.cols.saturating_sub(2); // 2 for the left and right corners
        let (mut first_part, first_part_len) = self.first_exited_held_title_part_full();
        let mut left_boundary =
            foreground_color(self.get_corner(boundary_type::BOTTOM_LEFT), self.color);
        let mut right_boundary =
            foreground_color(self.get_corner(boundary_type::BOTTOM_RIGHT), self.color);
        let res = if self.is_main_client {
            let (mut second_part, second_part_len) = self.second_held_title_part_full();
            let full_text_len = first_part_len + second_part_len;
            if full_text_len <= max_undertitle_length {
                // render exit status and tips
                let mut padding = String::new();
                for _ in full_text_len..max_undertitle_length {
                    padding.push_str(boundary_type::HORIZONTAL);
                }
                let mut ret = vec![];
                ret.append(&mut left_boundary);
                ret.append(&mut first_part);
                ret.append(&mut second_part);
                ret.append(&mut foreground_color(&padding, self.color));
                ret.append(&mut right_boundary);
                ret
            } else if first_part_len <= max_undertitle_length {
                // render only exit status
                let mut padding = String::new();
                for _ in first_part_len..max_undertitle_length {
                    padding.push_str(boundary_type::HORIZONTAL);
                }
                let mut ret = vec![];
                ret.append(&mut left_boundary);
                ret.append(&mut first_part);
                ret.append(&mut foreground_color(&padding, self.color));
                ret.append(&mut right_boundary);
                ret
            } else {
                self.empty_undertitle(max_undertitle_length)
            }
        } else {
            if first_part_len <= max_undertitle_length {
                // render first part
                let full_text_len = first_part_len;
                let mut padding = String::new();
                for _ in full_text_len..max_undertitle_length {
                    padding.push_str(boundary_type::HORIZONTAL);
                }
                let mut ret = vec![];
                ret.append(&mut left_boundary);
                ret.append(&mut first_part);
                ret.append(&mut foreground_color(&padding, self.color));
                ret.append(&mut right_boundary);
                ret
            } else {
                self.empty_undertitle(max_undertitle_length)
            }
        };
        Ok(res)
    }
    fn render_mouse_shortcuts_undertitle(&self) -> Result<Vec<TerminalCharacter>> {
        let max_undertitle_length = self.geom.cols.saturating_sub(2); // 2 for the left and right corners
        let mut left_boundary =
            foreground_color(self.get_corner(boundary_type::BOTTOM_LEFT), self.color);
        let mut right_boundary =
            foreground_color(self.get_corner(boundary_type::BOTTOM_RIGHT), self.color);
        let res = if self.is_main_client {
            self.empty_undertitle(max_undertitle_length)
        } else {
            let (mut hover_shortcuts, hover_shortcuts_len) = self.hover_shortcuts_part_full();
            if hover_shortcuts_len <= max_undertitle_length {
                // render exit status and tips
                let mut padding = String::new();
                for _ in hover_shortcuts_len..max_undertitle_length {
                    padding.push_str(boundary_type::HORIZONTAL);
                }
                let mut ret = vec![];
                ret.append(&mut left_boundary);
                ret.append(&mut hover_shortcuts);
                ret.append(&mut foreground_color(&padding, self.color));
                ret.append(&mut right_boundary);
                ret
            } else {
                self.empty_undertitle(max_undertitle_length)
            }
        };
        Ok(res)
    }
    pub fn clicked_on_pinned(&mut self, position: Position) -> bool {
        if self.is_floating {
            // TODO: this is not entirely accurate because our relative position calculation in
            // itself isn't - when that is fixed, we should adjust this as well
            let checkbox_center_position = self.geom.cols.saturating_sub(5);
            let checkbox_position_start = checkbox_center_position.saturating_sub(1);
            let checkbox_position_end = checkbox_center_position + 1;
            if position.line() == -1
                && (position.column() >= checkbox_position_start
                    && position.column() <= checkbox_position_end)
            {
                return true;
            }
        }
        false
    }
    pub fn render(&self) -> Result<(Vec<CharacterChunk>, Option<String>)> {
        let err_context = || "failed to render pane frame";
        let mut character_chunks = vec![];
        if self.geom.rows == 1 || !self.should_draw_pane_frames {
            // we do this explicitly when not drawing pane frames because this should only happen
            // if this is a stacked pane with pane frames off (and it doesn't necessarily have only
            // 1 row because it could also be a flexible stacked pane)
            // in this case we should always draw the pane title line, and only the title line
            let mut one_line_title = self.render_one_line_title().with_context(err_context)?;

            if self.content_offset.right != 0 && !self.should_draw_pane_frames {
                // here what happens is that the title should be offset to the right
                // in order to give room to the boundaries between the panes to be drawn
                one_line_title.pop();
            }
            let y_coords_of_title = if self.pane_is_stacked_under && !self.should_draw_pane_frames {
                // we only want to use the bottom offset in this case because panes that are
                // stacked above the flexible pane should actually appear exactly where they are on
                // screen, the content offset being "absorbed" by the flexible pane below them
                self.geom.y.saturating_sub(self.content_offset.bottom)
            } else {
                self.geom.y
            };

            character_chunks.push(CharacterChunk::new(
                one_line_title,
                self.geom.x,
                y_coords_of_title,
            ));
        } else {
            for row in 0..self.geom.rows {
                if row == 0 {
                    // top row
                    let title = self.render_title().with_context(err_context)?;
                    let x = self.geom.x;
                    let y = self.geom.y + row;
                    character_chunks.push(CharacterChunk::new(title, x, y));
                } else if row == self.geom.rows - 1 {
                    // bottom row
                    if self.mouse_is_hovering_over_pane && !self.is_main_client {
                        let x = self.geom.x;
                        let y = self.geom.y + row;
                        character_chunks.push(CharacterChunk::new(
                            self.render_mouse_shortcuts_undertitle()
                                .with_context(err_context)?,
                            x,
                            y,
                        ));
                    } else if self.exit_status.is_some() || self.is_first_run {
                        let x = self.geom.x;
                        let y = self.geom.y + row;
                        character_chunks.push(CharacterChunk::new(
                            self.render_held_undertitle().with_context(err_context)?,
                            x,
                            y,
                        ));
                    } else {
                        let mut bottom_row = vec![];
                        for col in 0..self.geom.cols {
                            let boundary = if col == 0 {
                                // bottom left corner
                                self.get_corner(boundary_type::BOTTOM_LEFT)
                            } else if col == self.geom.cols - 1 {
                                // bottom right corner
                                self.get_corner(boundary_type::BOTTOM_RIGHT)
                            } else {
                                boundary_type::HORIZONTAL
                            };

                            let mut boundary_character = foreground_color(boundary, self.color);
                            bottom_row.append(&mut boundary_character);
                        }
                        let x = self.geom.x;
                        let y = self.geom.y + row;
                        character_chunks.push(CharacterChunk::new(bottom_row, x, y));
                    }
                } else {
                    let boundary_character_left =
                        foreground_color(boundary_type::VERTICAL, self.color);
                    let boundary_character_right =
                        foreground_color(boundary_type::VERTICAL, self.color);

                    let x = self.geom.x;
                    let y = self.geom.y + row;
                    character_chunks.push(CharacterChunk::new(boundary_character_left, x, y));

                    let x = (self.geom.x + self.geom.cols).saturating_sub(1);
                    let y = self.geom.y + row;
                    character_chunks.push(CharacterChunk::new(boundary_character_right, x, y));
                }
            }
        }
        Ok((character_chunks, None))
    }
    fn first_exited_held_title_part_full(&self) -> (Vec<TerminalCharacter>, usize) {
        // (title part, length)
        match self.exit_status {
            Some(ExitStatus::Code(exit_code)) => {
                let mut first_part = vec![];
                let left_bracket = " [ ";
                let exited_text = "EXIT CODE: ";
                let exit_code_text = format!("{}", exit_code);
                let exit_code_color = if exit_code == 0 {
                    self.style.colors.exit_code_success.base
                } else {
                    self.style.colors.exit_code_error.base
                };
                let right_bracket = " ] ";
                first_part.append(&mut foreground_color(left_bracket, self.color));
                first_part.append(&mut foreground_color(exited_text, self.color));
                first_part.append(&mut foreground_color(
                    &exit_code_text,
                    Some(exit_code_color),
                ));
                first_part.append(&mut foreground_color(right_bracket, self.color));
                (
                    first_part,
                    left_bracket.len()
                        + exited_text.len()
                        + exit_code_text.len()
                        + right_bracket.len(),
                )
            },
            Some(ExitStatus::Exited) => {
                let mut first_part = vec![];
                let left_bracket = " [ ";
                let exited_text = "EXITED";
                let right_bracket = " ] ";
                first_part.append(&mut foreground_color(left_bracket, self.color));
                first_part.append(&mut foreground_color(
                    exited_text,
                    Some(self.style.colors.exit_code_error.base),
                ));
                first_part.append(&mut foreground_color(right_bracket, self.color));
                (
                    first_part,
                    left_bracket.len() + exited_text.len() + right_bracket.len(),
                )
            },
            None => (foreground_color(boundary_type::HORIZONTAL, self.color), 1),
        }
    }
    fn second_held_title_part_full(&self) -> (Vec<TerminalCharacter>, usize) {
        // (title part, length)
        let mut second_part = vec![];
        let left_enter_bracket = if self.is_first_run { " <" } else { "<" };
        let enter_text = "ENTER";
        let right_enter_bracket = ">";
        let enter_tip = if self.is_first_run {
            " run, "
        } else {
            " re-run, "
        };

        let left_esc_bracket = "<";
        let esc_text = "ESC";
        let right_esc_bracket = ">";
        let esc_tip = " drop to shell, ";

        let left_break_bracket = "<";
        let break_text = "Ctrl-c";
        let right_break_bracket = ">";
        let break_tip = " exit ";
        second_part.append(&mut foreground_color(left_enter_bracket, self.color));
        second_part.append(&mut foreground_color(
            enter_text,
            Some(self.style.colors.text_unselected.emphasis_0),
        ));
        second_part.append(&mut foreground_color(right_enter_bracket, self.color));
        second_part.append(&mut foreground_color(enter_tip, self.color));

        second_part.append(&mut foreground_color(left_esc_bracket, self.color));
        second_part.append(&mut foreground_color(
            esc_text,
            Some(self.style.colors.text_unselected.emphasis_0),
        ));
        second_part.append(&mut foreground_color(right_esc_bracket, self.color));
        second_part.append(&mut foreground_color(esc_tip, self.color));

        second_part.append(&mut foreground_color(left_break_bracket, self.color));
        second_part.append(&mut foreground_color(
            break_text,
            Some(self.style.colors.text_unselected.emphasis_0),
        ));
        second_part.append(&mut foreground_color(right_break_bracket, self.color));
        second_part.append(&mut foreground_color(break_tip, self.color));
        (
            second_part,
            left_enter_bracket.len()
                + enter_text.len()
                + right_enter_bracket.len()
                + enter_tip.len()
                + left_esc_bracket.len()
                + esc_text.len()
                + right_esc_bracket.len()
                + esc_tip.len()
                + left_break_bracket.len()
                + break_text.len()
                + right_break_bracket.len()
                + break_tip.len(),
        )
    }
    fn hover_shortcuts_part_full(&self) -> (Vec<TerminalCharacter>, usize) {
        // (title part, length)
        let mut hover_shortcuts = vec![];
        let alt_click_text = " Alt <Click>";
        let alt_click_tip = " - group,";
        let alt_right_click_text = " Alt <Right-Click>";
        let alt_right_click_tip = " - ungroup all ";

        hover_shortcuts.append(&mut foreground_color(alt_click_text, self.color));
        hover_shortcuts.append(&mut foreground_color(alt_click_tip, self.color));
        hover_shortcuts.append(&mut foreground_color(alt_right_click_text, self.color));
        hover_shortcuts.append(&mut foreground_color(alt_right_click_tip, self.color));
        (
            hover_shortcuts,
            alt_click_text.chars().count()
                + alt_click_tip.chars().count()
                + alt_right_click_text.chars().count()
                + alt_right_click_tip.chars().count(),
        )
    }
    fn empty_undertitle(&self, max_undertitle_length: usize) -> Vec<TerminalCharacter> {
        let mut left_boundary =
            foreground_color(self.get_corner(boundary_type::BOTTOM_LEFT), self.color);
        let mut right_boundary =
            foreground_color(self.get_corner(boundary_type::BOTTOM_RIGHT), self.color);
        let mut ret = vec![];
        let mut padding = String::new();
        for _ in 0..max_undertitle_length {
            padding.push_str(boundary_type::HORIZONTAL);
        }
        ret.append(&mut left_boundary);
        ret.append(&mut foreground_color(&padding, self.color));
        ret.append(&mut right_boundary);
        ret
    }
}
