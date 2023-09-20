use std::cell::RefCell;
use std::ops::Generator;
use std::rc::Rc;
use image::RgbImage;
// use std::sync::Arc;
// use clap::builder::ValueParserFactory;
// use yas::common::cancel::CancellationToken;
use yas::common::color::Color;
use yas::game_info::GameInfo;
use yas::inference::model::OCRModel;
use yas::capture::capture;
use yas::window_info::require_window_info::RequireWindowInfo;
use yas::window_info::window_info::WindowInfo;
use crate::scanner_controller::repository_layout::config::{GenshinRepositoryScannerLogicConfig};
use anyhow::{Result, anyhow};
use yas::common::positioning::{Pos, Rect, Size};
use yas::utils;
use log::{debug, info, error};
use std::time::SystemTime;
use yas::capture::capture::RelativeCapturable;
use yas::system_control::SystemControl;

#[derive(Debug)]
pub enum ScrollResult {
    TimeLimitExceeded,
    Interrupt,
    Success,
    Failed,
    Skip,
}

// todo use macros
struct GenshinRepositoryScanControllerWindowInfo {
    pub window_origin: Pos,
    pub panel_pos: Rect,
    pub flag_pos: Pos,
    pub item_gap: Size,
    pub item_size: Size,
    pub scan_margin: Pos,
    pub pool_pos: Rect,
}

impl From<&WindowInfo> for GenshinRepositoryScanControllerWindowInfo {
    fn from(value: &WindowInfo) -> Self {
        GenshinRepositoryScanControllerWindowInfo {
            window_origin: value.get::<Pos>("window_origin").unwrap(),
            panel_pos: value.get("genshin_repository_panel_pos").unwrap(),
            flag_pos: value.get("genshin_repository_flag_pos").unwrap(),
            item_gap: value.get("genshin_repository_item_gap").unwrap(),
            item_size: value.get("genshin_repository_item_size").unwrap(),
            scan_margin: value.get("genshin_repository_scan_margin").unwrap(),
            pool_pos: value.get("genshin_repository_pool_pos").unwrap(),
        }
    }
}

pub struct GenshinRepositoryScanController {
    // to detect whether an item changes
    pool: f64,

    initial_color: Color,

    // for scrolls
    scrolled_rows: u32,
    avg_scroll_one_row: f64,

    avg_switch_time: f64,
    scanned_count: usize,

    game_info: GameInfo,

    row: usize,
    col: usize,
    item_count: usize,

    config: GenshinRepositoryScannerLogicConfig,
    window_info: GenshinRepositoryScanControllerWindowInfo,
    system_control: SystemControl,
}

impl RequireWindowInfo for GenshinRepositoryScanController {
    fn require_window_info(window_info_builder: &mut yas::window_info::window_info_builder::WindowInfoBuilder) {
        window_info_builder
            .add_required_key("window_origin")
            .add_required_key("genshin_repository_panel_pos")
            .add_required_key("genshin_repository_flag_pos")
            .add_required_key("genshin_repository_item_gap")
            .add_required_key("genshin_repository_item_size")
            .add_required_key("genshin_repository_scan_margin")
            .add_required_key("genshin_repository_pool_pos")
            .add_required_key("genshin_repository_item_row")
            .add_required_key("genshin_repository_item_col");
    }
}

pub fn calc_pool(row: &Vec<u8>) -> f32 {
    let len = row.len() / 3;
    let mut pool: f32 = 0.0;

    for i in 0..len {
        pool += row[i * 3] as f32;
    }
    pool
}

// constructor
impl GenshinRepositoryScanController {
    pub fn new(config: GenshinRepositoryScannerLogicConfig, window_info: &WindowInfo, item_count: usize, game_info: GameInfo) -> Self {
        let item_row = window_info.get::<i32>("genshin_repository_item_row").unwrap();
        let item_col = window_info.get::<i32>("genshin_repository_item_col").unwrap();

        GenshinRepositoryScanController {
            system_control: SystemControl::new(),

            row: item_row as usize,
            col: item_col as usize,

            window_info: GenshinRepositoryScanControllerWindowInfo::from(window_info),
            config,

            pool: 0.0,

            initial_color: Color::new(0, 0, 0),

            scrolled_rows: 0,
            avg_scroll_one_row: 0.0,

            avg_switch_time: 0.0,
            // scanned_count: 0,

            game_info,
            item_count,
            scanned_count: 0,
        }
    }
}

impl GenshinRepositoryScanController {
    pub fn into_generator(object: Rc<RefCell<GenshinRepositoryScanController>>) -> impl Generator {
        // let mut_self = self.borrow_mut();
        let generator = move || {
            let mut scanned_row = 0;
            let mut scanned_count = 0;
            let mut start_row = 0;

            // let mut_self = || { return object.borrow_mut(); };
            // let immut_self = || { return object.borrow(); };

            let count = object.borrow().item_count;

            let total_row = (object.borrow().item_count + object.borrow().col - 1) / object.borrow().col;
            let last_row_col = if object.borrow().item_count % object.borrow().col == 0 {
                object.borrow().col
            } else {
                count % object.borrow().col
            };

            info!(
                "扫描任务共 {} 个物品，共计 {} 行，尾行 {} 个",
                count, total_row, last_row_col
            );

            object.borrow_mut().move_to(0, 0);

            #[cfg(target_os = "macos")]
            utils::sleep(20);

            // todo remove unwrap
            object.borrow_mut().system_control.mouse_click().unwrap();
            utils::sleep(1000);

            object.borrow_mut().sample_initial_color().unwrap();

            let row = object.borrow().row;

            'outer: while scanned_count < count {
                '_row: for row in start_row..row {
                    let row_item_count = if scanned_row == total_row - 1 {
                        last_row_col
                    } else {
                        object.borrow().col
                    };

                    '_col: for col in 0..row_item_count {
                        // 大于最大数量 或者 取消 或者 鼠标右键按下
                        // todo use controller
                        if utils::is_rmb_down() || scanned_count > count {
                            break 'outer;
                        }

                        object.borrow_mut().move_to(row, col);
                        object.borrow_mut().system_control.mouse_click().unwrap();

                        #[cfg(target_os = "macos")]
                        utils::sleep(20);

                        object.borrow_mut().wait_until_switched().unwrap();

                        // have to make sure at this point no mut ref exists
                        yield;

                        scanned_count += 1;
                        object.borrow_mut().scanned_count = scanned_count;
                    } // end '_col

                    scanned_row += 1;

                    // todo this is dangerous, use uniform integer type instead
                    if scanned_row >= object.borrow().config.max_row as usize {
                        info!("到达最大行数，准备退出……");
                        break 'outer;
                    }
                } // end '_row

                let remain = count - scanned_count;
                let remain_row = (remain + object.borrow().col - 1) / object.borrow().col;
                let scroll_row = remain_row.min(object.borrow().row);
                start_row = object.borrow().row - scroll_row;

                match object.borrow_mut().scroll_rows(scroll_row as i32) {
                    ScrollResult::TimeLimitExceeded => {
                        error!("翻页超时，扫描终止……");
                        break 'outer;
                    },
                    ScrollResult::Interrupt => break 'outer,
                    _ => (),
                }

                utils::sleep(100);
            }
        };

        generator
    }

    pub fn capture_panel(&self) -> Result<RgbImage> {
        self.window_info.panel_pos.capture_relative(self.window_info.window_origin)
    }

    #[inline(always)]
    pub fn get_flag_color(&self) -> Result<Color> {
        capture::get_color(self.window_info.flag_pos + self.window_info.window_origin)
    }

    #[inline(always)]
    pub fn sample_initial_color(&mut self) -> Result<()> {
        self.initial_color = self.get_flag_color()?;
        anyhow::Ok(())
    }

    pub fn align_row(&mut self) {
        for _ in 0..10 {
            let color = match self.get_flag_color() {
                Ok(color) => color,
                Err(_) => return,
            };

            if self.initial_color.distance(&color) > 10 {
                self.mouse_scroll(1, false);
                utils::sleep(self.config.scroll_delay);
            } else {
                break;
            }
        }
    }

    pub fn move_to(&mut self, row: usize, col: usize) {
        let (row, col) = (row as u32, col as u32);
        let origin = self.window_info.window_origin;

        let gap = self.window_info.item_gap;
        let margin = self.window_info.scan_margin;
        let size = self.window_info.item_size;

        let left = origin.x + margin.x + (gap.width + size.width) * (col as f64) + size.width / 2.0;
        let top = origin.y + margin.y + (gap.height + size.height) * (row as f64) + size.height / 2.0;

        self.system_control.mouse_move_to(left as i32, top as i32).unwrap();

        #[cfg(target_os = "macos")]
        utils::sleep(20);
    }

    pub fn scroll_one_row(&mut self) -> ScrollResult {
        let mut state = 0;

        for count in 0..25 {
            // if utils::is_rmb_down() || self.cancellation_token.cancelled() {
            //     return ScrollResult::Interrupt;
            // }
            if utils::is_rmb_down() {
                return ScrollResult::Interrupt;
            }

            // FIXME: Why -5 for windows?
            // #[cfg(windows)]
            // self.enigo.mouse_scroll_y(-5);

            self.mouse_scroll(1, count < 1);

            utils::sleep(self.config.scroll_delay);

            let color = match self.get_flag_color() {
                Ok(color) => color,
                Err(_) => return ScrollResult::Failed,
            };

            if state == 0 && self.initial_color.distance(&color) > 10 {
                state = 1;
            } else if state == 1 && self.initial_color.distance(&color) <= 10 {
                self.update_avg_row(count);
                return ScrollResult::Success;
            }
        }

        ScrollResult::TimeLimitExceeded
    }

    pub fn scroll_rows(&mut self, count: i32) -> ScrollResult {
        if cfg!(not(target_os = "macos")) && self.scrolled_rows >= 5 {
            let length = self.estimate_scroll_length(count);

            debug!(
                "Alread scrolled {} rows, estimated scroll length: {}",
                self.scrolled_rows, length
            );

            self.mouse_scroll(length, false);

            utils::sleep(400);

            self.align_row();
            return ScrollResult::Skip;
        }

        for _ in 0..count {
            match self.scroll_one_row() {
                ScrollResult::Success | ScrollResult::Skip => continue,
                v => {
                    info!("Scrolling failed: {:?}", v);
                    return v;
                },
            }
        }

        ScrollResult::Success
    }

    pub fn wait_until_switched(&mut self) -> Result<()> {
        if self.game_info.is_cloud {
            utils::sleep(self.config.cloud_wait_switch_item);
            return anyhow::Ok(());
        }

        let now = SystemTime::now();

        let mut consecutive_time = 0;
        let mut diff_flag = false;
        while now.elapsed().unwrap().as_millis() < self.config.max_wait_switch_item as u128 {
            let im: RgbImage = self.window_info.pool_pos
                .capture_relative(self.window_info.window_origin)?;

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
                    return anyhow::Ok(());
                }
            }
        }

        Err(anyhow!("Wait until switched failed"))
    }

    #[inline(always)]
    pub fn mouse_scroll(&mut self, length: i32, try_find: bool) {
        #[cfg(windows)]
        self.system_control.mouse_scroll(length, try_find).unwrap();

        #[cfg(target_os = "linux")]
        self.system_control.mouse_scroll(length, try_find);

        #[cfg(target_os = "macos")]
        {
            match self.game_info.ui {
                crate::common::UI::Desktop => {
                    self.system_control.mouse_scroll(length);
                    utils::sleep(20);
                },
                crate::common::UI::Mobile => {
                    if try_find {
                        self.system_control.mac_scroll_fast(length);
                    } else {
                        self.system_control.mac_scroll_slow(length);
                    }
                },
            }
        }
    }

    #[inline(always)]
    fn update_avg_row(&mut self, count: i32) {
        let current = self.avg_scroll_one_row * self.scrolled_rows as f64 + count as f64;
        self.scrolled_rows += 1;
        self.avg_scroll_one_row = current / self.scrolled_rows as f64;

        debug!(
            "avg scroll one row: {} ({})",
            self.avg_scroll_one_row, self.scrolled_rows
        );
    }

    #[inline(always)]
    fn estimate_scroll_length(&self, count: i32) -> i32 {
        ((self.avg_scroll_one_row * count as f64 - 2.0).round() as i32).max(0)
    }
}