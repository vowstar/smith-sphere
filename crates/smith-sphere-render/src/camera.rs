//! Orbit camera for the sphere view.
//!
//! The view axes follow the classic Smith chart at the home orientation:
//! `u` points right (open circuit on the right), `v` points up (inductive on
//! top) and `w` points at the viewer (positive-resistance hemisphere in front).

use smith_sphere_core::SpherePoint;

/// Yaw and pitch in degrees plus a zoom factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self::HOME
    }
}

impl Camera {
    /// Home view: mostly the positive hemisphere with the boundary visible.
    pub const HOME: Self = Self {
        yaw_deg: -52.0,
        pitch_deg: 24.0,
        zoom: 1.0,
    };
    pub const MIN_ZOOM: f32 = 0.6;
    pub const MAX_ZOOM: f32 = 3.0;

    /// Faces the positive-resistance hemisphere, matching the right chart.
    #[must_use]
    pub fn positive_view(self) -> Self {
        Self {
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            zoom: self.zoom,
        }
    }

    /// Faces the negative-resistance hemisphere from behind.
    #[must_use]
    pub fn negative_view(self) -> Self {
        Self {
            yaw_deg: 180.0,
            pitch_deg: 0.0,
            zoom: self.zoom,
        }
    }

    /// Camera that brings `point` to the centre of the visible disc.
    #[must_use]
    pub fn looking_at(self, point: SpherePoint) -> Self {
        let yaw = (-point.u).atan2(point.w);
        let horizontal = (point.u * point.u + point.w * point.w).sqrt();
        let pitch = point.v.atan2(horizontal);
        Self {
            yaw_deg: yaw.to_degrees() as f32,
            pitch_deg: pitch.to_degrees() as f32,
            zoom: self.zoom,
        }
    }

    /// Applies a drag delta in points so that the surface under the pointer
    /// follows it: dragging right turns the front of the sphere to the right,
    /// dragging down tilts the front downward. Measured against screenshots,
    /// the same signs hold for mouse and touch input.
    pub fn orbit(&mut self, delta_x: f32, delta_y: f32) {
        self.yaw_deg = wrap_degrees(self.yaw_deg + delta_x * 0.45);
        self.pitch_deg = (self.pitch_deg + delta_y * 0.45).clamp(-89.0, 89.0);
    }

    pub fn zoom_by(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
    }

    /// Moves a fraction of the way toward another camera; used for short
    /// animated transitions.
    #[must_use]
    pub fn approach(self, target: Self, fraction: f32) -> Self {
        let yaw_delta = wrap_degrees(target.yaw_deg - self.yaw_deg);
        Self {
            yaw_deg: wrap_degrees(self.yaw_deg + yaw_delta * fraction),
            pitch_deg: self.pitch_deg + (target.pitch_deg - self.pitch_deg) * fraction,
            zoom: self.zoom + (target.zoom - self.zoom) * fraction,
        }
    }

    /// Angular distance to another camera in degrees.
    #[must_use]
    pub fn distance_to(self, other: Self) -> f32 {
        wrap_degrees(other.yaw_deg - self.yaw_deg).abs() + (other.pitch_deg - self.pitch_deg).abs()
    }

    /// Projects a sphere point into view space: `x` right, `y` up, `depth`
    /// toward the viewer. All three components stay within `[-1, 1]`.
    #[must_use]
    pub fn view(self, point: SpherePoint) -> [f32; 3] {
        let (sin_yaw, cos_yaw) = self.yaw_deg.to_radians().sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch_deg.to_radians().sin_cos();
        let u = point.u as f32;
        let v = point.v as f32;
        let w = point.w as f32;
        let x = u * cos_yaw + w * sin_yaw;
        let depth_after_yaw = -u * sin_yaw + w * cos_yaw;
        let y = v * cos_pitch - depth_after_yaw * sin_pitch;
        let depth = v * sin_pitch + depth_after_yaw * cos_pitch;
        [x, y, depth]
    }
}

fn wrap_degrees(angle: f32) -> f32 {
    let mut wrapped = angle % 360.0;
    if wrapped > 180.0 {
        wrapped -= 360.0;
    } else if wrapped < -180.0 {
        wrapped += 360.0;
    }
    wrapped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_orientation_matches_the_smith_chart_axes() {
        let camera = Camera::HOME.positive_view();
        let open = camera.view(SpherePoint::OPEN);
        assert!(open[0] > 0.999, "open circuit on the right");
        let inductive = camera.view(SpherePoint::INDUCTIVE_UNIT);
        assert!(inductive[1] > 0.999, "inductive on top");
        let matched = camera.view(SpherePoint::MATCH);
        assert!(matched[2] > 0.999, "positive hemisphere faces the viewer");
    }

    #[test]
    fn negative_view_faces_the_negative_hemisphere() {
        let camera = Camera::HOME.negative_view();
        assert!(camera.view(SpherePoint::NEGATIVE_MATCH)[2] > 0.999);
        assert!(
            camera.view(SpherePoint::INDUCTIVE_UNIT)[1] > 0.999,
            "inductive stays on top"
        );
        assert!(
            camera.view(SpherePoint::OPEN)[0] < -0.999,
            "open circuit appears on the left from behind"
        );
    }

    #[test]
    fn looking_at_brings_any_point_to_the_front() {
        for point in [
            SpherePoint::SHORT,
            SpherePoint::NEGATIVE_MATCH,
            SpherePoint::CAPACITIVE_UNIT,
            SpherePoint {
                u: 0.3,
                v: -0.5,
                w: -0.812_403_84,
            }
            .renormalized(),
        ] {
            let camera = Camera::HOME.looking_at(point);
            let view = camera.view(point);
            assert!(view[2] > 0.999, "{point:?} -> {view:?}");
            assert!(view[0].abs() < 1e-3 && view[1].abs() < 1e-3);
        }
    }

    #[test]
    fn view_preserves_length() {
        let camera = Camera {
            yaw_deg: 47.0,
            pitch_deg: -31.0,
            zoom: 1.0,
        };
        let view = camera.view(SpherePoint {
            u: 0.6,
            v: 0.0,
            w: 0.8,
        });
        let length = (view[0] * view[0] + view[1] * view[1] + view[2] * view[2]).sqrt();
        assert!((length - 1.0).abs() < 1e-5);
    }
}
