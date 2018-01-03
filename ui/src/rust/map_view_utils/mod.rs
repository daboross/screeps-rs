pub const ZOOM_MODIFIER: f64 = 1.0 / 500.0;
pub const MIN_ZOOM: f64 = 0.05;
pub const MAX_ZOOM: f64 = 10.0;

#[inline(always)]
pub fn zoom_multiplier_from_factor(zoom_factor: f64) -> f64 {
    zoom_factor.powf(2.0)
}

#[inline(always)]
pub fn bound_zoom(zoom_factor: f64) -> f64 {
    zoom_factor.powf(2.0).min(MAX_ZOOM).max(MIN_ZOOM).powf(0.5)
}
