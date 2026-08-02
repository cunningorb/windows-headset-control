//! Direct2D / DirectWrite backend.
//!
//! Walks the primitive list `layout` produced and draws it. Decides nothing:
//! every position and colour arrives already resolved, which is what keeps the
//! interesting logic testable without a GPU.
//!
//! Renders into a WIC bitmap rather than straight to a window, because the panel
//! is a layered window: the finished premultiplied-alpha image is handed to
//! `UpdateLayeredWindow`, which is what lets the drop shadow, the rounded
//! corners and the knob glow composite over whatever is behind them.

#![allow(unsafe_code)]

use windows::core::{Result, HSTRING};
use windows::Win32::Foundation::GENERIC_READ;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN, D2D1_GRADIENT_STOP,
    D2D1_PIXEL_FORMAT, D2D_POINT_2F, D2D_RECT_F,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1RenderTarget, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
    D2D1_BRUSH_PROPERTIES, D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_GAMMA_2_2, D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICBitmap, IWICBitmapFrameEncode,
    IWICImagingFactory, WICBitmapCacheOnLoad, WICBitmapEncoderNoCache, WICBitmapLockRead, WICRect,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};

use crate::ui::layout::{Align, Panel, Primitive, Rect};
use crate::ui::theme::{self, Color};

/// Everything expensive, created once and reused for every repaint.
pub struct Renderer {
    d2d: ID2D1Factory,
    dwrite: IDWriteFactory,
    wic: IWICImagingFactory,
}

/// A rendered panel: premultiplied BGRA, ready for `UpdateLayeredWindow`.
pub struct RenderedPanel {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

impl Renderer {
    pub fn new() -> Result<Renderer> {
        unsafe {
            let d2d: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let wic: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            Ok(Renderer { d2d, dwrite, wic })
        }
    }

    /// Draws a panel, including the drop-shadow margin around it.
    pub fn render(&self, panel: &Panel, scale: f32) -> Result<RenderedPanel> {
        let shadow = theme::SHADOW;
        let w = ((theme::PANEL_W + shadow * 2.0) * scale).ceil() as u32;
        let h = ((panel.height + shadow * 2.0) * scale).ceil() as u32;

        unsafe {
            let bitmap: IWICBitmap = self.wic.CreateBitmap(
                w,
                h,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapCacheOnLoad,
            )?;

            let props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0 * scale,
                dpiY: 96.0 * scale,
                ..Default::default()
            };
            let rt: ID2D1RenderTarget = self.d2d.CreateWicBitmapRenderTarget(&bitmap, &props)?;
            rt.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);

            rt.BeginDraw();
            rt.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }));

            self.draw_shadow(&rt, panel.height)?;
            for prim in &panel.primitives {
                self.draw(&rt, prim, shadow)?;
            }
            rt.EndDraw(None, None)?;

            // Copy the pixels out for UpdateLayeredWindow / PNG encoding.
            let rect = WICRect {
                X: 0,
                Y: 0,
                Width: w as i32,
                Height: h as i32,
            };
            let lock = bitmap.Lock(&rect, WICBitmapLockRead.0 as u32)?;
            let mut size = 0u32;
            let mut ptr = std::ptr::null_mut();
            lock.GetDataPointer(&mut size, &mut ptr)?;
            let bgra = std::slice::from_raw_parts(ptr, size as usize).to_vec();

            Ok(RenderedPanel {
                width: w,
                height: h,
                bgra,
            })
        }
    }

    /// A soft dark halo behind the panel.
    ///
    /// The mockups' margin is a CSS drop shadow over a page; on a desktop it has
    /// to composite over whatever is actually behind the window, which is why it
    /// is drawn into the alpha channel rather than faked with a solid colour.
    unsafe fn draw_shadow(&self, rt: &ID2D1RenderTarget, panel_h: f32) -> Result<()> {
        let s = theme::SHADOW;
        for i in 0..(s as i32) {
            let t = i as f32;
            let spread = s - t;
            let alpha = 0.045 * (1.0 - t / s);
            let brush = rt.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: alpha,
                },
                None,
            )?;
            let rr = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: s - spread,
                    top: s - spread + 2.0,
                    right: s + theme::PANEL_W + spread,
                    bottom: s + panel_h + spread + 2.0,
                },
                radiusX: theme::PANEL_RADIUS + spread,
                radiusY: theme::PANEL_RADIUS + spread,
            };
            rt.FillRoundedRectangle(&rr, &brush);
        }
        Ok(())
    }

    unsafe fn brush(
        &self,
        rt: &ID2D1RenderTarget,
        c: Color,
    ) -> Result<windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush> {
        let (r, g, b, a) = c.as_f32();
        rt.CreateSolidColorBrush(&D2D1_COLOR_F { r, g, b, a }, None)
    }

    unsafe fn draw(&self, rt: &ID2D1RenderTarget, prim: &Primitive, off: f32) -> Result<()> {
        match prim {
            Primitive::RoundRect {
                rect,
                radius,
                fill,
                stroke,
                stroke_w,
            } => {
                let rr = D2D1_ROUNDED_RECT {
                    rect: to_d2d(rect, off),
                    radiusX: *radius,
                    radiusY: *radius,
                };
                if let Some(c) = fill {
                    let b = self.brush(rt, *c)?;
                    rt.FillRoundedRectangle(&rr, &b);
                }
                if let Some(c) = stroke {
                    let b = self.brush(rt, *c)?;
                    rt.DrawRoundedRectangle(&rr, &b, *stroke_w, None);
                }
            }
            Primitive::Circle {
                cx,
                cy,
                r,
                fill,
                stroke,
                stroke_w,
            } => {
                let e = D2D1_ELLIPSE {
                    point: D2D_POINT_2F {
                        x: cx + off,
                        y: cy + off,
                    },
                    radiusX: *r,
                    radiusY: *r,
                };
                if let Some(c) = fill {
                    let b = self.brush(rt, *c)?;
                    rt.FillEllipse(&e, &b);
                }
                if let Some(c) = stroke {
                    let b = self.brush(rt, *c)?;
                    rt.DrawEllipse(&e, &b, *stroke_w, None);
                }
            }
            Primitive::Glow { cx, cy, r, color } => {
                let (cr, cg, cb, ca) = color.as_f32();
                let stops = [
                    D2D1_GRADIENT_STOP {
                        position: 0.0,
                        color: D2D1_COLOR_F {
                            r: cr,
                            g: cg,
                            b: cb,
                            a: ca,
                        },
                    },
                    D2D1_GRADIENT_STOP {
                        position: 1.0,
                        color: D2D1_COLOR_F {
                            r: cr,
                            g: cg,
                            b: cb,
                            a: 0.0,
                        },
                    },
                ];
                let coll = rt.CreateGradientStopCollection(
                    &stops,
                    D2D1_GAMMA_2_2,
                    D2D1_EXTEND_MODE_CLAMP,
                )?;
                let props = D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
                    center: D2D_POINT_2F {
                        x: cx + off,
                        y: cy + off,
                    },
                    gradientOriginOffset: D2D_POINT_2F { x: 0.0, y: 0.0 },
                    radiusX: *r,
                    radiusY: *r,
                };
                let brush = rt.CreateRadialGradientBrush(
                    &props,
                    Some(&D2D1_BRUSH_PROPERTIES {
                        opacity: 1.0,
                        transform: windows::Foundation::Numerics::Matrix3x2::identity(),
                    }),
                    &coll,
                )?;
                let e = D2D1_ELLIPSE {
                    point: D2D_POINT_2F {
                        x: cx + off,
                        y: cy + off,
                    },
                    radiusX: *r,
                    radiusY: *r,
                };
                rt.FillEllipse(&e, &brush);
            }
            Primitive::Line {
                x0,
                y0,
                x1,
                y1,
                w,
                color,
            } => {
                let b = self.brush(rt, *color)?;
                rt.DrawLine(
                    D2D_POINT_2F {
                        x: x0 + off,
                        y: y0 + off,
                    },
                    D2D_POINT_2F {
                        x: x1 + off,
                        y: y1 + off,
                    },
                    &b,
                    *w,
                    None,
                );
            }
            Primitive::Path {
                points,
                closed,
                fill,
                stroke,
                stroke_w,
            } => {
                if points.len() < 2 {
                    return Ok(());
                }
                let geo = self.d2d.CreatePathGeometry()?;
                let sink = geo.Open()?;
                sink.BeginFigure(
                    D2D_POINT_2F {
                        x: points[0].0 + off,
                        y: points[0].1 + off,
                    },
                    if fill.is_some() {
                        D2D1_FIGURE_BEGIN_FILLED
                    } else {
                        D2D1_FIGURE_BEGIN_HOLLOW
                    },
                );
                let rest: Vec<D2D_POINT_2F> = points[1..]
                    .iter()
                    .map(|(x, y)| D2D_POINT_2F {
                        x: x + off,
                        y: y + off,
                    })
                    .collect();
                sink.AddLines(&rest);
                sink.EndFigure(if *closed {
                    D2D1_FIGURE_END_CLOSED
                } else {
                    D2D1_FIGURE_END_OPEN
                });
                sink.Close()?;
                if let Some(c) = fill {
                    let b = self.brush(rt, *c)?;
                    rt.FillGeometry(&geo, &b, None);
                }
                if let Some(c) = stroke {
                    let b = self.brush(rt, *c)?;
                    rt.DrawGeometry(&geo, &b, *stroke_w, None);
                }
            }
            Primitive::Text {
                rect,
                text,
                size,
                weight,
                color,
                align,
                tracking,
            } => {
                let fmt = self.text_format(*size, *weight, *align)?;
                let b = self.brush(rt, *color)?;
                // Letter-spacing is emulated by inserting thin spaces, which is
                // enough for the two tracked-out captions and avoids the
                // typography API for a cosmetic detail.
                let shown = if *tracking > 0.5 {
                    text.chars()
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>()
                        .join("\u{2009}")
                } else {
                    text.clone()
                };
                let wide: Vec<u16> = shown.encode_utf16().collect();
                rt.DrawText(
                    &wide,
                    &fmt,
                    &to_d2d(rect, off),
                    &b,
                    windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                    windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        }
        Ok(())
    }

    unsafe fn text_format(
        &self,
        size: f32,
        weight: u32,
        align: Align,
    ) -> Result<IDWriteTextFormat> {
        let fmt = self.dwrite.CreateTextFormat(
            &HSTRING::from(theme::FONT_FAMILY),
            None,
            DWRITE_FONT_WEIGHT(weight as i32),
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size * 96.0 / 72.0,
            &HSTRING::from("en-us"),
        )?;
        fmt.SetTextAlignment(match align {
            Align::Left => DWRITE_TEXT_ALIGNMENT_LEADING,
            Align::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
            Align::Right => DWRITE_TEXT_ALIGNMENT_TRAILING,
        })?;
        fmt.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        Ok(fmt)
    }

    /// Writes a rendered panel to a PNG.
    ///
    /// Exists so panel appearance can be diffed against the mockups without a
    /// window or a headset, which is how "as close as possible" gets to be a
    /// number instead of an opinion.
    pub fn save_png(&self, img: &RenderedPanel, path: &str) -> Result<()> {
        unsafe {
            let stream = self.wic.CreateStream()?;
            stream.InitializeFromFilename(&HSTRING::from(path), GENERIC_READ.0 | 0x4000_0000)?;
            let encoder = self.wic.CreateEncoder(
                &windows::Win32::Graphics::Imaging::GUID_ContainerFormatPng,
                std::ptr::null(),
            )?;
            encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
            let mut frame: Option<IWICBitmapFrameEncode> = None;
            let mut options = None;
            encoder.CreateNewFrame(&mut frame, &mut options)?;
            let frame = frame.expect("CreateNewFrame yields a frame on success");
            frame.Initialize(None)?;
            frame.SetSize(img.width, img.height)?;
            let mut fmt = GUID_WICPixelFormat32bppPBGRA;
            frame.SetPixelFormat(&mut fmt)?;
            frame.WritePixels(img.height, img.width * 4, &img.bgra)?;
            frame.Commit()?;
            encoder.Commit()?;
            Ok(())
        }
    }
}

fn to_d2d(r: &Rect, off: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: r.x + off,
        top: r.y + off,
        right: r.x + r.w + off,
        bottom: r.y + r.h + off,
    }
}
