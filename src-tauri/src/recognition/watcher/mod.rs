//! Recognition watcher 模块
//!
//! 拆分自原 watcher.rs 单文件（1815 行）为三个子模块：
//! - `manager` — watcher 生命周期（restart/stop/run 循环）
//! - `matching` — NCC 图像匹配 + 颜色匹配算法
//! - `capture`  — 截图 + 参考图像 I/O + base64 编码

pub(crate) mod capture;
pub(crate) mod manager;
pub(crate) mod matching;

// Re-export 公开接口，保持与旧 watcher.rs 兼容的导入路径
pub(crate) use capture::{capture_region, load_reference_image, read_reference_image_as_data_url};
pub(crate) use manager::{restart_watchers, start_activation_session, stop_all_watchers};
pub(crate) use matching::{aggregate_probe_hits_pub, compare_images, probe_hit_targets};

#[cfg(test)]
mod tests {
    use super::capture::base64_encode;
    use super::manager::watcher_should_run;
    use super::matching::{
        average_region_rgb, color_distance, compare_images, match_color_probes, probe_hit,
        scan_region_for_color,
    };
    use crate::morse::types::RegionRect;
    use crate::recognition::types::{ColorMatchMethod, ColorMatchMode, ColorProbe, ColorTarget};
    use image::{DynamicImage, GrayImage, Luma, Rgb, RgbImage, Rgba, RgbaImage};

    /// 辅助：取比较结果的 similarity
    fn score(a: &DynamicImage, b: &DynamicImage) -> f32 {
        compare_images(a, b).similarity
    }

    // ---- 全局总开关门控 ----

    #[test]
    fn watcher_should_run_enabled_when_both_on() {
        assert!(watcher_should_run(true, true));
    }

    #[test]
    fn watcher_should_run_disabled_when_global_off() {
        assert!(!watcher_should_run(false, true));
    }

    #[test]
    fn watcher_should_run_disabled_when_recognition_off() {
        assert!(!watcher_should_run(true, false));
    }

    #[test]
    fn watcher_should_run_disabled_when_both_off() {
        assert!(!watcher_should_run(false, false));
    }

    // ---- NCC 图像比较 ----

    #[test]
    fn same_image_returns_near_one() {
        let img = RgbaImage::from_pixel(4, 4, Rgba([128, 64, 32, 255]));
        let a = DynamicImage::ImageRgba8(img.clone());
        let b = DynamicImage::ImageRgba8(img);
        let s = score(&a, &b);
        assert!(s > 0.99, "同一图像 NCC 应接近 1.0，实际 {}", s);
    }

    #[test]
    fn uniform_image_returns_one() {
        let img = GrayImage::from_pixel(4, 4, Luma([100]));
        let a = DynamicImage::ImageLuma8(img.clone());
        let b = DynamicImage::ImageLuma8(img);
        let s = score(&a, &b);
        assert_eq!(s, 1.0, "均匀图像应返回 1.0");
    }

    #[test]
    fn different_images_returns_low() {
        let mut a = RgbaImage::new(4, 4);
        let mut b = RgbaImage::new(4, 4);
        for (_x, y, pixel) in a.enumerate_pixels_mut() {
            *pixel = if y < 2 {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([0, 0, 0, 255])
            };
        }
        for (_x, y, pixel) in b.enumerate_pixels_mut() {
            *pixel = if y < 2 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            };
        }
        let s = score(&DynamicImage::ImageRgba8(a), &DynamicImage::ImageRgba8(b));
        assert!(s < 0.1, "反差图像相似度应很低，实际 {}", s);
    }

    #[test]
    fn alpha_transparent_pixels_excluded() {
        let mut reference = RgbaImage::new(4, 4);
        for (_x, y, pixel) in reference.enumerate_pixels_mut() {
            if y < 2 {
                *pixel = Rgba([0, 0, 0, 0]);
            } else {
                *pixel = Rgba([128, 128, 128, 255]);
            }
        }
        let screenshot = RgbImage::from_pixel(4, 4, Rgb([128, 128, 128]));
        let s = score(
            &DynamicImage::ImageRgb8(screenshot),
            &DynamicImage::ImageRgba8(reference),
        );
        assert!(s > 0.99, "透明像素应被排除，剩余像素完全匹配，实际 {}", s);
    }

    #[test]
    fn zero_width_returns_zero() {
        let a = DynamicImage::ImageRgba8(RgbaImage::new(0, 4));
        let b = DynamicImage::ImageRgba8(RgbaImage::new(0, 4));
        let s = score(&a, &b);
        assert_eq!(s, 0.0, "零宽度应返回 0.0");
    }

    #[test]
    fn reference_larger_than_screenshot_returns_zero() {
        let screenshot =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(10, 10, Rgba([100, 100, 100, 255])));
        let reference =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(20, 20, Rgba([100, 100, 100, 255])));
        let result = compare_images(&screenshot, &reference);
        assert_eq!(result.similarity, 0.0, "参考图比截图大应返回 0.0");
    }

    #[test]
    fn template_match_finds_offset_reference() {
        let mut screenshot = RgbImage::from_pixel(100, 100, Rgb([50, 50, 50]));
        for y in 20..30 {
            for x in 30..40 {
                let color = if (x + y) % 2 == 0 {
                    Rgb([200, 100, 50])
                } else {
                    Rgb([50, 150, 200])
                };
                screenshot.put_pixel(x, y, color);
            }
        }

        let mut reference = RgbImage::new(10, 10);
        for y in 0..10u32 {
            for x in 0..10u32 {
                let color = if (x + 30 + y + 20) % 2 == 0 {
                    Rgb([200, 100, 50])
                } else {
                    Rgb([50, 150, 200])
                };
                reference.put_pixel(x, y, color);
            }
        }

        let result = compare_images(
            &DynamicImage::ImageRgb8(screenshot),
            &DynamicImage::ImageRgb8(reference),
        );
        assert!(
            result.similarity > 0.9,
            "模板匹配应找到偏移的参考图，实际 {}",
            result.similarity
        );
        assert!(
            (result.best_x as i32 - 30).abs() <= 3,
            "最佳匹配 X 坐标应接近 30，实际 {}",
            result.best_x
        );
        assert!(
            (result.best_y as i32 - 20).abs() <= 3,
            "最佳匹配 Y 坐标应接近 20，实际 {}",
            result.best_y
        );
    }

    #[test]
    fn rgb_ncc_rejects_grayscale_alias() {
        let mut a = RgbaImage::new(8, 8);
        for (x, _y, pixel) in a.enumerate_pixels_mut() {
            if x < 4 {
                *pixel = Rgba([200, 50, 50, 255]);
            } else {
                *pixel = Rgba([50, 50, 200, 255]);
            }
        }
        let mut b = RgbaImage::new(8, 8);
        for (x, _y, pixel) in b.enumerate_pixels_mut() {
            if x < 4 {
                *pixel = Rgba([50, 200, 50, 255]);
            } else {
                *pixel = Rgba([200, 50, 200, 255]);
            }
        }
        let s = score(&DynamicImage::ImageRgba8(a), &DynamicImage::ImageRgba8(b));
        assert!(s < 0.7, "RGB NCC 应能区分不同颜色排列，实际 {}", s);
    }

    #[test]
    fn same_size_rgb_ncc_high() {
        let img = RgbImage::from_pixel(10, 10, Rgb([128, 64, 200]));
        let a = DynamicImage::ImageRgb8(img.clone());
        let b = DynamicImage::ImageRgb8(img);
        let result = compare_images(&a, &b);
        assert!(
            result.similarity > 0.99,
            "同尺寸同图像 RGB NCC 应接近 1.0，实际 {}",
            result.similarity
        );
    }

    #[test]
    fn template_match_with_alpha_mask() {
        let mut reference = RgbaImage::new(8, 8);
        for y in 0..8 {
            for x in 0..8 {
                if x >= 2 && x < 6 && y >= 2 && y < 6 {
                    reference.put_pixel(x, y, Rgba([200, 100, 50, 255]));
                } else {
                    reference.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                }
            }
        }

        let mut screenshot = RgbImage::from_pixel(40, 40, Rgb([30, 30, 30]));
        for y in 12..20 {
            for x in 10..18 {
                if x >= 12 && x < 16 && y >= 14 && y < 18 {
                    screenshot.put_pixel(x, y, Rgb([200, 100, 50]));
                } else {
                    screenshot.put_pixel(x, y, Rgb([100, 200, 80]));
                }
            }
        }

        let result = compare_images(
            &DynamicImage::ImageRgb8(screenshot),
            &DynamicImage::ImageRgba8(reference),
        );
        assert!(
            result.similarity > 0.8,
            "Alpha mask 模板匹配应能找到目标，实际 {}",
            result.similarity
        );
    }

    // ---- base64 编码 ----

    #[test]
    fn base64_encode_empty() {
        assert_eq!(base64_encode(&[]), "");
    }

    #[test]
    fn base64_encode_hello() {
        assert_eq!(base64_encode(b"Hello"), "SGVsbG8=");
    }

    #[test]
    fn base64_encode_padding() {
        assert_eq!(base64_encode(b"A"), "QQ==");
        assert_eq!(base64_encode(b"AB"), "QUI=");
    }

    // ---- 颜色距离 ----

    #[test]
    fn color_distance_zero_for_same_color() {
        assert_eq!(color_distance([100, 100, 100], [100, 100, 100]), 0.0);
    }

    #[test]
    fn color_distance_orthogonal_channels() {
        assert!((color_distance([130, 100, 100], [100, 100, 100]) - 30.0).abs() < 0.01);
        let d = color_distance([110, 110, 110], [100, 100, 100]);
        assert!((d - 17.32).abs() < 0.1, "实际 {}", d);
    }

    // ---- 区域平均 RGB ----

    #[test]
    fn average_region_rgb_uniform() {
        let img = RgbaImage::from_pixel(3, 3, Rgba([10, 20, 30, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        assert_eq!(average_region_rgb(&dyn_img), [10, 20, 30]);
    }

    #[test]
    fn average_region_rgb_mixed() {
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([100, 0, 0, 255]));
        img.put_pixel(0, 1, Rgba([0, 100, 0, 255]));
        img.put_pixel(1, 1, Rgba([100, 100, 0, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        assert_eq!(average_region_rgb(&dyn_img), [50, 50, 0]);
    }

    #[test]
    fn average_region_rgb_ignores_alpha() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([200, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 0]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        assert_eq!(average_region_rgb(&dyn_img), [200, 0, 0]);
    }

    // ---- match_color_probes ----

    #[test]
    fn match_color_probes_all_mode_all_hit() {
        let screenshots = vec![
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([200, 100, 50, 255]))),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255]))),
        ];
        let probes = vec![
            ColorProbe {
                region: Some(RegionRect {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 2,
                }),
                targets: vec![ColorTarget {
                    color: [200, 100, 50],
                    tolerance: 10,
                }],
                probe_match_mode: ColorMatchMode::Any,
                legacy_target_color: None,
                legacy_tolerance: None,
            },
            ColorProbe {
                region: Some(RegionRect {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 2,
                }),
                targets: vec![ColorTarget {
                    color: [10, 20, 30],
                    tolerance: 10,
                }],
                probe_match_mode: ColorMatchMode::Any,
                legacy_target_color: None,
                legacy_tolerance: None,
            },
        ];
        let result = match_color_probes(
            &screenshots,
            &probes,
            ColorMatchMode::All,
            ColorMatchMethod::Average,
        );
        assert!(result.matched, "All 模式全命中应触发");
        assert_eq!(result.hit_count, 2);
    }

    #[test]
    fn match_color_probes_all_mode_partial_miss() {
        let screenshots = vec![
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([200, 100, 50, 255]))),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255]))),
        ];
        let probes = vec![
            ColorProbe {
                region: Some(RegionRect {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 2,
                }),
                targets: vec![ColorTarget {
                    color: [200, 100, 50],
                    tolerance: 10,
                }],
                probe_match_mode: ColorMatchMode::Any,
                legacy_target_color: None,
                legacy_tolerance: None,
            },
            ColorProbe {
                region: Some(RegionRect {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 2,
                }),
                targets: vec![ColorTarget {
                    color: [10, 20, 30],
                    tolerance: 10,
                }],
                probe_match_mode: ColorMatchMode::Any,
                legacy_target_color: None,
                legacy_tolerance: None,
            },
        ];
        let result = match_color_probes(
            &screenshots,
            &probes,
            ColorMatchMode::All,
            ColorMatchMethod::Average,
        );
        assert!(!result.matched, "All 模式部分未命中不应触发");
        assert_eq!(result.hit_count, 1);
    }

    #[test]
    fn match_color_probes_any_mode_one_hit() {
        let screenshots = vec![
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([200, 100, 50, 255]))),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255]))),
        ];
        let probes = vec![
            ColorProbe {
                region: Some(RegionRect {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 2,
                }),
                targets: vec![ColorTarget {
                    color: [200, 100, 50],
                    tolerance: 10,
                }],
                probe_match_mode: ColorMatchMode::Any,
                legacy_target_color: None,
                legacy_tolerance: None,
            },
            ColorProbe {
                region: Some(RegionRect {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 2,
                }),
                targets: vec![ColorTarget {
                    color: [10, 20, 30],
                    tolerance: 10,
                }],
                probe_match_mode: ColorMatchMode::Any,
                legacy_target_color: None,
                legacy_tolerance: None,
            },
        ];
        let result = match_color_probes(
            &screenshots,
            &probes,
            ColorMatchMode::Any,
            ColorMatchMethod::Average,
        );
        assert!(result.matched, "Any 模式任一命中即触发");
        assert_eq!(result.hit_count, 1);
    }

    #[test]
    fn match_color_probes_empty_returns_false() {
        let result = match_color_probes(&[], &[], ColorMatchMode::All, ColorMatchMethod::Average);
        assert!(!result.matched, "无探针不应触发");
        assert_eq!(result.hit_count, 0);
    }

    #[test]
    fn match_color_probes_any_pixel_method_hits() {
        let mut img = RgbaImage::new(3, 3);
        for y in 0..3 {
            for x in 0..3 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        img.put_pixel(1, 1, Rgba([255, 0, 0, 255]));
        let screenshots = vec![DynamicImage::ImageRgba8(img)];
        let probes = vec![ColorProbe {
            region: Some(RegionRect {
                x: 0,
                y: 0,
                width: 3,
                height: 3,
            }),
            targets: vec![ColorTarget {
                color: [255, 0, 0],
                tolerance: 10,
            }],
            probe_match_mode: ColorMatchMode::Any,
            legacy_target_color: None,
            legacy_tolerance: None,
        }];
        let avg = match_color_probes(
            &screenshots,
            &probes,
            ColorMatchMode::All,
            ColorMatchMethod::Average,
        );
        assert!(!avg.matched, "average 模式平均色为黑，距红远，未中");
        let any = match_color_probes(
            &screenshots,
            &probes,
            ColorMatchMode::All,
            ColorMatchMethod::AnyPixel,
        );
        assert!(any.matched, "anyPixel 模式存在红像素应命中");
        assert_eq!(any.hit_count, 1);
    }

    #[test]
    fn match_color_probes_any_pixel_combined_with_mode_all() {
        let mut img1 = RgbaImage::new(2, 2);
        for y in 0..2 {
            for x in 0..2 {
                img1.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }
        let img2 = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255]));
        let screenshots = vec![
            DynamicImage::ImageRgba8(img1),
            DynamicImage::ImageRgba8(img2),
        ];
        let probes = vec![
            ColorProbe {
                region: Some(RegionRect {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 2,
                }),
                targets: vec![ColorTarget {
                    color: [255, 0, 0],
                    tolerance: 10,
                }],
                probe_match_mode: ColorMatchMode::Any,
                legacy_target_color: None,
                legacy_tolerance: None,
            },
            ColorProbe {
                region: Some(RegionRect {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 2,
                }),
                targets: vec![ColorTarget {
                    color: [255, 0, 0],
                    tolerance: 10,
                }],
                probe_match_mode: ColorMatchMode::Any,
                legacy_target_color: None,
                legacy_tolerance: None,
            },
        ];
        let all = match_color_probes(
            &screenshots,
            &probes,
            ColorMatchMode::All,
            ColorMatchMethod::AnyPixel,
        );
        assert!(!all.matched, "mode=All 第二探针未中，不触发");
        assert_eq!(all.hit_count, 1);
        let any = match_color_probes(
            &screenshots,
            &probes,
            ColorMatchMode::Any,
            ColorMatchMethod::AnyPixel,
        );
        assert!(any.matched, "mode=Any 任一命中即触发");
    }

    // ---- scan_region_for_color ----

    #[test]
    fn any_pixel_hit_single_pixel() {
        let mut img = RgbaImage::new(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        img.put_pixel(2, 1, Rgba([200, 100, 50, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        let result = scan_region_for_color(&dyn_img, [200, 100, 50], 10.0, false);
        assert!(result.matching_count >= 1, "应至少命中 1 像素");
        assert_eq!(result.nearest_color, [200, 100, 50], "最近像素应为目标色");
    }

    #[test]
    fn any_pixel_no_hit_returns_nearest() {
        let img = RgbaImage::from_pixel(3, 3, Rgba([0, 0, 0, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        let result = scan_region_for_color(&dyn_img, [255, 255, 255], 10.0, false);
        assert_eq!(result.matching_count, 0, "无命中像素");
        assert_eq!(result.nearest_color, [0, 0, 0], "最近像素为黑");
    }

    #[test]
    fn any_pixel_early_exit_count_only() {
        let img = RgbaImage::from_pixel(2, 2, Rgba([200, 100, 50, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        let result = scan_region_for_color(&dyn_img, [200, 100, 50], 10.0, true);
        assert_eq!(
            result.matching_count, 1,
            "count_only=true 命中后应早退，count 为 1"
        );
    }

    #[test]
    fn any_pixel_tolerance_boundary() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([110, 100, 100, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        let result = scan_region_for_color(&dyn_img, [100, 100, 100], 10.0, false);
        assert!(result.matching_count >= 1, "距离恰等于容差应命中");
    }

    #[test]
    fn any_pixel_returns_closest_matching_position() {
        let mut img = RgbaImage::from_pixel(3, 3, Rgba([0, 0, 0, 255]));
        img.put_pixel(0, 2, Rgba([105, 100, 100, 255]));
        img.put_pixel(2, 0, Rgba([101, 100, 100, 255]));
        let result = scan_region_for_color(
            &DynamicImage::ImageRgba8(img),
            [100, 100, 100],
            10.0,
            true,
        );

        assert_eq!(result.match_position, Some((2, 0)));
    }

    #[test]
    fn match_color_probes_keeps_match_position() {
        let mut img = RgbaImage::from_pixel(3, 3, Rgba([0, 0, 0, 255]));
        img.put_pixel(1, 2, Rgba([100, 100, 100, 255]));
        let probes = vec![ColorProbe {
            region: Some(RegionRect {
                x: 10,
                y: 20,
                width: 3,
                height: 3,
            }),
            targets: vec![ColorTarget {
                color: [100, 100, 100],
                tolerance: 0,
            }],
            probe_match_mode: ColorMatchMode::Any,
            legacy_target_color: None,
            legacy_tolerance: None,
        }];
        let result = match_color_probes(
            &[DynamicImage::ImageRgba8(img)],
            &probes,
            ColorMatchMode::All,
            ColorMatchMethod::AnyPixel,
        );

        assert_eq!(result.matched_probes[0].match_position, Some((1, 2)));
    }

    #[test]
    fn multi_target_probe_uses_closest_matched_position() {
        let mut img = RgbaImage::from_pixel(3, 1, Rgba([0, 0, 0, 255]));
        img.put_pixel(0, 0, Rgba([102, 0, 0, 255]));
        img.put_pixel(2, 0, Rgba([0, 100, 0, 255]));
        let image = DynamicImage::ImageRgba8(img);

        for probe_match_mode in [ColorMatchMode::Any, ColorMatchMode::All] {
            let probe = ColorProbe {
                region: None,
                targets: vec![
                    ColorTarget {
                        color: [100, 0, 0],
                        tolerance: 5,
                    },
                    ColorTarget {
                        color: [0, 100, 0],
                        tolerance: 0,
                    },
                ],
                probe_match_mode,
                legacy_target_color: None,
                legacy_tolerance: None,
            };

            let hit = probe_hit(&image, &probe, ColorMatchMethod::AnyPixel, true);
            assert!(hit.matched);
            assert_eq!(hit.match_position, Some((2, 0)));
        }
    }

    // ---- probe_hit ----

    #[test]
    fn probe_hit_average_uses_region_avg() {
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 255]));
        img.put_pixel(0, 1, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 1, Rgba([0, 0, 0, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        let probe = ColorProbe {
            region: Some(RegionRect {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            }),
            targets: vec![ColorTarget {
                color: [127, 0, 0],
                tolerance: 5,
            }],
            probe_match_mode: ColorMatchMode::Any,
            legacy_target_color: None,
            legacy_tolerance: None,
        };
        let hit = probe_hit(&dyn_img, &probe, ColorMatchMethod::Average, false);
        assert!(hit.matched, "average 模式平均色应命中");
        assert_eq!(
            hit.matching_pixel_count, 0,
            "average 模式 matching_pixel_count 恒 0"
        );
    }

    #[test]
    fn probe_hit_any_pixel_finds_single_pixel() {
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 255]));
        img.put_pixel(0, 1, Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 1, Rgba([0, 0, 0, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        let probe = ColorProbe {
            region: Some(RegionRect {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            }),
            targets: vec![ColorTarget {
                color: [255, 0, 0],
                tolerance: 10,
            }],
            probe_match_mode: ColorMatchMode::Any,
            legacy_target_color: None,
            legacy_tolerance: None,
        };
        let avg_hit = probe_hit(&dyn_img, &probe, ColorMatchMethod::Average, false);
        assert!(
            !avg_hit.matched,
            "average 模式平均色 [127,0,0] 距 [255,0,0] 远，应未中"
        );
        let any_hit = probe_hit(&dyn_img, &probe, ColorMatchMethod::AnyPixel, false);
        assert!(any_hit.matched, "anyPixel 模式存在红像素应命中");
        assert!(any_hit.matching_pixel_count >= 1, "anyPixel 命中数应 >= 1");
    }

    // ---- A-M1 修复验证：watcher_should_run 实时读取 ----

    #[test]
    fn watcher_should_run_uses_realtime_recognition_state_not_snapshot() {
        // 验证 watcher_should_run 接受实时参数而非快照
        assert!(watcher_should_run(true, true));
        assert!(!watcher_should_run(true, false));
        assert!(!watcher_should_run(false, true));
    }

    // ---- T-8: Watcher 循环集成测试 ----

    /// 核心：匹配触发 playback，不匹配不触发
    /// 这里验证 compare_images 的匹配/不匹配逻辑（watcher 循环的核心决策函数）
    #[test]
    fn watcher_loop_matching_triggers_playback() {
        // 构造完全相同的截图和参考图 → 相似度接近 1.0 → 匹配
        let img = RgbaImage::from_pixel(10, 10, Rgba([200, 100, 50, 255]));
        let screenshot = DynamicImage::ImageRgba8(img.clone());
        let reference = DynamicImage::ImageRgba8(img);
        let result = compare_images(&screenshot, &reference);
        assert!(
            result.similarity >= 0.75,
            "匹配图像相似度应 >= 0.75（默认阈值），实际 {}",
            result.similarity
        );
        // 模拟 watcher 循环决策：similarity >= threshold → 触发 playback
        let threshold = 0.75;
        let should_trigger = result.similarity >= threshold;
        assert!(should_trigger, "匹配成功应触发 playback");
    }

    #[test]
    fn watcher_loop_no_match_skips_playback() {
        // 构造差异巨大的截图和参考图 → 相似度很低 → 不匹配
        let mut a = RgbaImage::new(10, 10);
        let mut b = RgbaImage::new(10, 10);
        for y in 0..10 {
            for x in 0..10 {
                a.put_pixel(x, y, Rgba([255, 255, 255, 255]));
                b.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let screenshot = DynamicImage::ImageRgba8(a);
        let reference = DynamicImage::ImageRgba8(b);
        let result = compare_images(&screenshot, &reference);
        assert!(
            result.similarity < 0.75,
            "差异巨大图像相似度应 < 0.75，实际 {}",
            result.similarity
        );
        // 模拟 watcher 循环决策：similarity < threshold → 不触发 playback
        let threshold = 0.75;
        let should_trigger = result.similarity >= threshold;
        assert!(!should_trigger, "不匹配不应触发 playback");
    }

    /// 识色 watcher 循环集成：匹配触发，不匹配不触发
    #[test]
    fn color_watcher_loop_matching_triggers_playback() {
        // 构造颜色探针完全匹配的截图
        let screenshots = vec![DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            4,
            4,
            Rgba([200, 100, 50, 255]),
        ))];
        let probes = vec![ColorProbe {
            region: Some(RegionRect {
                x: 0,
                y: 0,
                width: 4,
                height: 4,
            }),
            targets: vec![ColorTarget {
                color: [200, 100, 50],
                tolerance: 10,
            }],
            probe_match_mode: ColorMatchMode::Any,
            legacy_target_color: None,
            legacy_tolerance: None,
        }];
        let result = match_color_probes(
            &screenshots,
            &probes,
            ColorMatchMode::All,
            ColorMatchMethod::Average,
        );
        assert!(result.matched, "颜色匹配应触发 playback");
    }

    #[test]
    fn color_watcher_loop_no_match_skips_playback() {
        // 构造颜色探针不匹配的截图
        let screenshots = vec![DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            4,
            4,
            Rgba([0, 0, 0, 255]),
        ))];
        let probes = vec![ColorProbe {
            region: Some(RegionRect {
                x: 0,
                y: 0,
                width: 4,
                height: 4,
            }),
            targets: vec![ColorTarget {
                color: [200, 100, 50],
                tolerance: 10,
            }],
            probe_match_mode: ColorMatchMode::Any,
            legacy_target_color: None,
            legacy_tolerance: None,
        }];
        let result = match_color_probes(
            &screenshots,
            &probes,
            ColorMatchMode::All,
            ColorMatchMethod::Average,
        );
        assert!(!result.matched, "颜色不匹配不应触发 playback");
    }

    /// 验证 watcher_should_run 在全局/识别触发开关变化时的即时响应
    #[test]
    fn watcher_loop_respects_toggle_changes_immediately() {
        // 模拟 watcher 循环中开关状态变化序列：
        // tick 1: 两者都开 → 执行截图匹配
        assert!(watcher_should_run(true, true));
        // tick 2: 用户关闭识别触发模块开关 → 立即跳过
        assert!(!watcher_should_run(true, false));
        // tick 3: 用户关闭全局开关 → 也跳过
        assert!(!watcher_should_run(false, true));
        // tick 4: 两者都关 → 跳过
        assert!(!watcher_should_run(false, false));
        // tick 5: 用户重新打开 → 恢复执行
        assert!(watcher_should_run(true, true));
    }

    // ---- T-8: region_watcher_step / color_watcher_step 集成测试 ----

    /// Mock 实现 WatcherDeps，记录所有调用。
    struct MockWatcherDeps {
        capture_result: Option<image::DynamicImage>,
        compare_result: f32,
        dispatched: std::sync::Mutex<Vec<crate::recognition::player::AudioCommand>>,
    }

    impl MockWatcherDeps {
        fn new(capture_result: Option<image::DynamicImage>, compare_result: f32) -> Self {
            Self {
                capture_result,
                compare_result,
                dispatched: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    impl super::manager::WatcherDeps for MockWatcherDeps {
        fn capture(
            &self,
            _region: &crate::morse::types::RegionRect,
        ) -> Option<image::DynamicImage> {
            self.capture_result.clone()
        }

        fn compare(
            &self,
            _screenshot: &image::DynamicImage,
            _reference: &image::DynamicImage,
        ) -> f32 {
            self.compare_result
        }

        fn dispatch_playback(&self, command: crate::recognition::player::AudioCommand) {
            self.dispatched.lock().unwrap().push(command);
        }
    }

    /// VAL-AR-022: region_watcher_step 在 recognition_on=true 时匹配成功并分派回放。
    #[test]
    fn region_watcher_step_dispatches_playback_on_match() {
        let reference =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 4, Rgba([200, 100, 50, 255])));
        let screenshot = reference.clone();
        let deps = MockWatcherDeps::new(Some(screenshot), 0.95);
        let (tx, _rx) = std::sync::mpsc::channel();
        let region = RegionRect {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        };
        let resolved = crate::recognition::ResolvedPlay {
            path: "/test/audio.wav".to_string(),
            volume: 0.8,
            allow_simultaneous: false,
        };

        let result = super::manager::region_watcher_step(
            &deps,
            true, // global_on
            true, // recognition_on
            &region,
            &reference,
            0.75, // threshold
            "test-card",
            &tx,
            Some(&resolved),
        );

        assert!(result, "匹配成功时应返回 true");
        assert_eq!(
            deps.dispatched.lock().unwrap().len(),
            1,
            "应分派 1 个回放命令"
        );
    }

    /// VAL-AR-022: region_watcher_step 在 recognition_on=false 时跳过分派。
    #[test]
    fn region_watcher_step_skips_when_recognition_off() {
        let reference =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 4, Rgba([200, 100, 50, 255])));
        let screenshot = reference.clone();
        let deps = MockWatcherDeps::new(Some(screenshot), 0.95);
        let (tx, _rx) = std::sync::mpsc::channel();
        let region = RegionRect {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        };

        let result = super::manager::region_watcher_step(
            &deps,
            true,  // global_on
            false, // recognition_on = false → 跳过
            &region,
            &reference,
            0.75,
            "test-card",
            &tx,
            None,
        );

        assert!(!result, "recognition_off 时应返回 false");
        assert!(
            deps.dispatched.lock().unwrap().is_empty(),
            "recognition_off 时不应分派回放"
        );
    }

    /// VAL-AR-022: 模拟循环中切换 recognition_enabled，先开后关再开。
    #[test]
    fn region_watcher_step_flips_recognition_enabled_mid_loop() {
        let reference =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 4, Rgba([200, 100, 50, 255])));
        let screenshot = reference.clone();
        let (tx, _rx) = std::sync::mpsc::channel();
        let region = RegionRect {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        };
        let resolved = crate::recognition::ResolvedPlay {
            path: "/test/audio.wav".to_string(),
            volume: 0.8,
            allow_simultaneous: false,
        };

        // tick 1: recognition_on=true → 匹配成功 → 分派
        let deps = MockWatcherDeps::new(Some(screenshot.clone()), 0.95);
        let result1 = super::manager::region_watcher_step(
            &deps,
            true,
            true,
            &region,
            &reference,
            0.75,
            "card-1",
            &tx,
            Some(&resolved),
        );
        assert!(result1, "tick 1: recognition_on=true 应匹配");
        assert_eq!(deps.dispatched.lock().unwrap().len(), 1);

        // tick 2: recognition_on=false → 跳过（模拟用户中途关闭识别触发开关）
        let deps2 = MockWatcherDeps::new(Some(screenshot.clone()), 0.95);
        let result2 = super::manager::region_watcher_step(
            &deps2,
            true,
            false,
            &region,
            &reference,
            0.75,
            "card-1",
            &tx,
            Some(&resolved),
        );
        assert!(!result2, "tick 2: recognition_on=false 应跳过");
        assert!(deps2.dispatched.lock().unwrap().is_empty());

        // tick 3: recognition_on=true → 恢复分派
        let deps3 = MockWatcherDeps::new(Some(screenshot), 0.95);
        let result3 = super::manager::region_watcher_step(
            &deps3,
            true,
            true,
            &region,
            &reference,
            0.75,
            "card-1",
            &tx,
            Some(&resolved),
        );
        assert!(result3, "tick 3: recognition_on=true 应恢复匹配");
        assert_eq!(deps3.dispatched.lock().unwrap().len(), 1);
    }

    /// VAL-AR-022: color_watcher_step 在 recognition_on=false 时跳过分派。
    #[test]
    fn color_watcher_step_skips_when_recognition_off() {
        let screenshots = vec![DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            4,
            4,
            Rgba([200, 100, 50, 255]),
        ))];
        let probes = vec![ColorProbe {
            region: Some(RegionRect {
                x: 0,
                y: 0,
                width: 4,
                height: 4,
            }),
            targets: vec![ColorTarget {
                color: [200, 100, 50],
                tolerance: 10,
            }],
            probe_match_mode: ColorMatchMode::Any,
            legacy_target_color: None,
            legacy_tolerance: None,
        }];
        let deps = MockWatcherDeps::new(None, 0.0); // 不重要，color_watcher_step 不使用 capture/compare
        let (tx, _rx) = std::sync::mpsc::channel();

        let result = super::manager::color_watcher_step(
            &deps,
            true,  // global_on
            false, // recognition_on = false
            &screenshots,
            &probes,
            &ColorMatchMode::All,
            &ColorMatchMethod::Average,
            &tx,
            None,
        );

        assert!(!result, "recognition_off 时应返回 false");
        assert!(
            deps.dispatched.lock().unwrap().is_empty(),
            "recognition_off 时不应分派回放"
        );
    }

    /// VAL-AR-022: color_watcher_step 在匹配成功且 recognition_on=true 时分派回放。
    #[test]
    fn color_watcher_step_dispatches_playback_on_match() {
        let screenshots = vec![DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            4,
            4,
            Rgba([200, 100, 50, 255]),
        ))];
        let probes = vec![ColorProbe {
            region: Some(RegionRect {
                x: 0,
                y: 0,
                width: 4,
                height: 4,
            }),
            targets: vec![ColorTarget {
                color: [200, 100, 50],
                tolerance: 10,
            }],
            probe_match_mode: ColorMatchMode::Any,
            legacy_target_color: None,
            legacy_tolerance: None,
        }];
        let deps = MockWatcherDeps::new(None, 0.0);
        let (tx, _rx) = std::sync::mpsc::channel();
        let resolved = crate::recognition::ResolvedPlay {
            path: "/test/audio.wav".to_string(),
            volume: 0.8,
            allow_simultaneous: false,
        };

        let result = super::manager::color_watcher_step(
            &deps,
            true, // global_on
            true, // recognition_on
            &screenshots,
            &probes,
            &ColorMatchMode::All,
            &ColorMatchMethod::Average,
            &tx,
            Some(&resolved),
        );

        assert!(result, "匹配成功且 recognition_on=true 时应返回 true");
        assert_eq!(
            deps.dispatched.lock().unwrap().len(),
            1,
            "应分派 1 个回放命令"
        );
    }
}
