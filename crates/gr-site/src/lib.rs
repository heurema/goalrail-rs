#![deny(unsafe_op_in_unsafe_fn, unreachable_pub)]

#[unsafe(no_mangle)]
pub extern "C" fn ease_out_cubic(progress: f64) -> f64 {
    let progress = progress.clamp(0.0, 1.0);
    1.0 - (1.0 - progress).powi(3)
}

#[unsafe(no_mangle)]
pub extern "C" fn signal_radius(progress: f64) -> f64 {
    let progress = progress.clamp(0.0, 1.0);
    let distance = (progress - 0.5) / 0.1;
    3.2 + 1.6 * (-(distance.powi(2))).exp()
}

#[unsafe(no_mangle)]
pub extern "C" fn midpoint_opacity(progress: f64) -> f64 {
    let progress = progress.clamp(0.0, 1.0);
    let fade_in = ((progress - 0.34) / 0.12).clamp(0.0, 1.0);
    let fade_out = ((progress - 0.8) / 0.16).clamp(0.0, 1.0);
    fade_in * (1.0 - 0.5 * fade_out)
}

#[cfg(test)]
mod tests {
    use super::{ease_out_cubic, midpoint_opacity, signal_radius};

    #[test]
    fn easing_is_bounded_and_reaches_both_ends() {
        assert_eq!(ease_out_cubic(-1.0), 0.0);
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        assert_eq!(ease_out_cubic(2.0), 1.0);
        assert!(ease_out_cubic(0.5) > 0.5);
    }

    #[test]
    fn signal_expands_at_the_agent_handoff() {
        assert!((signal_radius(0.5) - 4.8).abs() < 1e-12);

        let shoulder = 3.2 + 1.6 * (-1.0_f64).exp();
        assert!((signal_radius(0.4) - shoulder).abs() < 1e-12);
        assert!((signal_radius(0.6) - shoulder).abs() < 1e-12);
    }

    #[test]
    fn midpoint_label_fades_in_then_settles() {
        assert_eq!(midpoint_opacity(0.0), 0.0);
        assert!((midpoint_opacity(0.4) - 0.5).abs() < 1e-12);
        assert!(midpoint_opacity(0.5) > 0.9);
        assert_eq!(midpoint_opacity(1.0), 0.5);
    }
}
