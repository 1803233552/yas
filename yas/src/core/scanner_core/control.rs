use super::*;
use enigo::MouseControllable;
use image::RgbImage;
use std::time::SystemTime;

#[cfg(target_os = "macos")]
use crate::common::utils::*;

impl ScannerCore {
    pub fn align_row(&mut self) {
        let mut count = 0;

        while count < 10 {
            if self.get_flag_color().unwrap() == self.initial_color {
                return;
            }

            #[cfg(windows)]
            self.enigo.mouse_scroll_y(-1);

            #[cfg(target_os = "linux")]
            self.enigo.mouse_scroll_y(1);

            #[cfg(target_os = "macos")]
            {
                match self.game_info.ui {
                    crate::common::UI::Desktop => {
                        self.enigo.mouse_scroll_y(-1);
                        utils::sleep(20);
                    },
                    crate::common::UI::Mobile => {
                        mac_scroll(&mut self.enigo, 1);
                    },
                }
            }

            utils::sleep(self.config.scroll_delay);
            count += 1;
        }
    }

    pub fn move_to(&mut self, row: usize, col: usize) {
        let (row, col) = (row as u32, col as u32);
        let origin = &self.scan_info.origin;

        let gap = &self.scan_info.item_gap;
        let margin = &self.scan_info.scan_margin;
        let size = &self.scan_info.item_size;

        let left = origin.x + margin.width + (gap.width + size.width) * col + size.width / 2;
        let top = origin.y + margin.height + (gap.height + size.height) * row + size.height / 4;

        self.enigo.mouse_move_to(left as i32, top as i32);

        #[cfg(target_os = "macos")]
        utils::sleep(20);
    }

    pub fn scroll_one_row(&mut self) -> ScrollResult {
        let mut state = 0;
        let mut count = 0;
        let max_scroll = 25;
        while count < max_scroll {
            if utils::is_rmb_down() || self.cancellation_token.cancelled() {
                return ScrollResult::Interrupt;
            }

            #[cfg(windows)]
            self.enigo.mouse_scroll_y(-5);
            #[cfg(target_os = "linux")]
            self.enigo.mouse_scroll_y(1);
            #[cfg(target_os = "macos")]
            {
                match self.game_info.ui {
                    crate::common::UI::Desktop => {
                        self.enigo.mouse_scroll_y(-1);
                        // utils::sleep(20);
                    },
                    crate::common::UI::Mobile => {
                        mac_scroll(&mut self.enigo, 1);
                    },
                }
            }
            utils::sleep(self.config.scroll_delay);
            count += 1;
            if let Ok(color) = self.get_flag_color() {
                if state == 0 && color != self.initial_color {
                    state = 1;
                } else if state == 1 && self.initial_color == color {
                    self.avg_scroll_one_row = (self.avg_scroll_one_row * self.scrolled_rows as f64
                        + count as f64)
                        / (self.scrolled_rows as f64 + 1.0);
                    info!("Avg scroll/row: {}", self.avg_scroll_one_row);
                    self.scrolled_rows += 1;
                    return ScrollResult::Success;
                }
            } else {
                return ScrollResult::Failed;
            }
        }

        ScrollResult::TimeLimitExceeded
    }

    pub fn scroll_rows(&mut self, count: usize) -> ScrollResult {
        if self.scrolled_rows >= 5 {
            let scroll = ((self.avg_scroll_one_row * count as f64 - 3.0).round() as u32).max(0);
            for _ in 0..scroll {
                #[cfg(windows)]
                self.enigo.mouse_scroll_y(-1);
                #[cfg(target_os = "linux")]
                self.enigo.mouse_scroll_y(1);
                #[cfg(target_os = "macos")]
                {
                    // self.enigo.mouse_scroll_y(1);
                    // mac_scroll(&mut self.enigo, 1);
                    match self.game_info.ui {
                        crate::common::UI::Desktop => {
                            self.enigo.mouse_scroll_y(-1);
                            utils::sleep(20);
                        },
                        crate::common::UI::Mobile => {
                            mac_scroll(&mut self.enigo, 1);
                        },
                    }
                }

                if self.cancellation_token.cancelled() {
                    return ScrollResult::Interrupt;
                }
            }

            utils::sleep(400);

            self.align_row();
            return ScrollResult::Skip;
        }

        for _ in 0..count {
            match self.scroll_one_row() {
                ScrollResult::TimeLimitExceeded => return ScrollResult::TimeLimitExceeded,
                ScrollResult::Interrupt => return ScrollResult::Interrupt,
                _ => (),
            }
        }

        ScrollResult::Success
    }

    pub fn wait_until_switched(&mut self) -> bool {
        if self.game_info.is_cloud {
            utils::sleep(self.config.cloud_wait_switch_item);
            return true;
        }

        let now = SystemTime::now();

        let mut consecutive_time = 0;
        let mut diff_flag = false;
        while now.elapsed().unwrap().as_millis() < self.config.max_wait_switch_item as u128 {
            let im: RgbImage = Rect::from(&self.scan_info.pool_pos)
                .capture_relative(&self.scan_info.origin)
                .unwrap();

            let pool = calc_pool(im.as_raw()) as f64;

            if (pool - self.pool).abs() > 0.000001 {
                self.pool = pool;
                diff_flag = true;
                consecutive_time = 0;
            } else if diff_flag {
                consecutive_time += 1;
                if consecutive_time == 1 {
                    self.avg_switch_time = (self.avg_switch_time * self.scanned_count as f64
                        + now.elapsed().unwrap().as_millis() as f64)
                        / (self.scanned_count as f64 + 1.0);
                    self.scanned_count += 1;
                    return true;
                }
            }
        }

        false
    }
}
