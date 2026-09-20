//! Animation and frame management for flicker-free terminal graphics.
//!
//! This module provides the core infrastructure for building smooth, professional-quality
//! terminal animations. The key component is [`FrameBuffer`], which implements double buffering
//! to eliminate visual tearing and flickering during frame updates.
//!
//! # Double Buffering Explained
//!
//! Double buffering is a technique where two buffers are maintained:
//! - **Front buffer**: Currently displayed on screen (read-only during drawing)
//! - **Back buffer**: Being prepared for the next frame (where you draw)
//!
//! The workflow is:
//! 1. Clear the back buffer
//! 2. Draw the next frame to the back buffer
//! 3. Instantly swap front and back buffers (O(1) pointer swap)
//! 4. Render the new front buffer to the terminal
//! 5. Repeat
//!
//! This eliminates flickering because the user only sees complete frames - never
//! partially drawn content.
//!
//! # Example
//!
//! ```no_run
//! use dotmax::animation::FrameBuffer;
//! use dotmax::TerminalRenderer;
//!
//! // Create a double-buffered frame system
//! let mut buffer = FrameBuffer::new(80, 24);
//!
//! // Get mutable access to the back buffer for drawing
//! let back = buffer.get_back_buffer();
//! back.clear();
//! back.set_dot(10, 10).unwrap();  // Draw something
//!
//! // Swap buffers - instant O(1) operation
//! buffer.swap_buffers();
//!
//! // Render the front buffer to terminal
//! // let mut renderer = TerminalRenderer::new().unwrap();
//! // buffer.render(&mut renderer).unwrap();
//! ```
//!
//! # Performance
//!
//! - Buffer swap time: <1ms (pointer swap, not data copy)
//! - Designed for 60+ fps animations
//! - Memory efficient: buffers are reused, not reallocated

mod differential;
mod frame_buffer;
mod loop_helper;
mod prerender;
mod timing;

pub use differential::DifferentialRenderer;
pub use frame_buffer::FrameBuffer;
pub use loop_helper::{AnimationLoop, AnimationLoopBuilder};
pub use prerender::PrerenderedAnimation;
pub use timing::FrameTimer;
