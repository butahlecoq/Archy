"""Immutable plan geometry used by brief evaluation and workflow adapters."""

from dataclasses import dataclass
import math
from typing import Any, Mapping


SETBACK_TOLERANCE_M = 1e-9


@dataclass(frozen=True)
class Point2:
    """A point in the site's east/north coordinate plane, measured in metres."""

    x_m: float
    y_m: float


@dataclass(frozen=True)
class Bounds2:
    """Axis-aligned bounds in the site's east/north coordinate plane."""

    min_x_m: float
    max_x_m: float
    min_y_m: float
    max_y_m: float

    def as_mapping(self) -> dict[str, float]:
        """Return the stable report representation for these bounds."""
        return {
            "west_m": self.min_x_m,
            "east_m": self.max_x_m,
            "south_m": self.min_y_m,
            "north_m": self.max_y_m,
        }


@dataclass(frozen=True)
class Footprint:
    """A rectangular footprint rotated about its local lower-left corner."""

    width_m: float
    depth_m: float
    origin_x_m: float
    origin_y_m: float
    rotation_degrees: float = 0.0

    def __post_init__(self) -> None:
        _finite_positive(self.width_m, "width_m")
        _finite_positive(self.depth_m, "depth_m")
        _finite_nonnegative(self.origin_x_m, "origin_x_m")
        _finite_nonnegative(self.origin_y_m, "origin_y_m")
        if (
            isinstance(self.rotation_degrees, bool)
            or not isinstance(self.rotation_degrees, (int, float))
            or not math.isfinite(self.rotation_degrees)
            or not 0 <= self.rotation_degrees < 360
        ):
            raise ValueError("rotation_degrees must be finite and in [0, 360)")
        object.__setattr__(self, "width_m", float(self.width_m))
        object.__setattr__(self, "depth_m", float(self.depth_m))
        object.__setattr__(self, "origin_x_m", float(self.origin_x_m))
        object.__setattr__(self, "origin_y_m", float(self.origin_y_m))
        object.__setattr__(self, "rotation_degrees", float(self.rotation_degrees))

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "Footprint":
        """Parse the brief/report footprint fields, defaulting omitted angle to zero."""
        if not isinstance(value, Mapping):
            raise ValueError("footprint must be a mapping")
        return cls(
            width_m=_finite_positive(value.get("width_m"), "width_m"),
            depth_m=_finite_positive(value.get("depth_m"), "depth_m"),
            origin_x_m=_finite_nonnegative(value.get("origin_x_m"), "origin_x_m"),
            origin_y_m=_finite_nonnegative(value.get("origin_y_m"), "origin_y_m"),
            rotation_degrees=_rotation(value.get("rotation_degrees", 0.0)),
        )

    def center(self) -> Point2:
        """Return the world-space rectangle center."""
        local_center = Point2(self.width_m / 2, self.depth_m / 2)
        return self._transform(local_center)

    def corners(self) -> tuple[Point2, Point2, Point2, Point2]:
        """Return transformed local corners in counter-clockwise order."""
        return (
            self._transform(Point2(0.0, 0.0)),
            self._transform(Point2(self.width_m, 0.0)),
            self._transform(Point2(self.width_m, self.depth_m)),
            self._transform(Point2(0.0, self.depth_m)),
        )

    def bounds(self) -> Bounds2:
        """Return the AABB derived from the four world-space corners."""
        corners = self.corners()
        return Bounds2(
            min(point.x_m for point in corners),
            max(point.x_m for point in corners),
            min(point.y_m for point in corners),
            max(point.y_m for point in corners),
        )

    def as_mapping(self) -> dict[str, float]:
        """Return normalized input fields for a new evaluation report."""
        return {
            "width_m": self.width_m,
            "depth_m": self.depth_m,
            "origin_x_m": self.origin_x_m,
            "origin_y_m": self.origin_y_m,
            "rotation_degrees": self.rotation_degrees,
        }

    def _transform(self, point: Point2) -> Point2:
        cosine, sine = _rotation_components(self.rotation_degrees)
        return Point2(
            self.origin_x_m + point.x_m * cosine - point.y_m * sine,
            self.origin_y_m + point.x_m * sine + point.y_m * cosine,
        )


@dataclass(frozen=True)
class BuildableArea:
    """Closed rectangular setback area with a small coordinate tolerance."""

    west_m: float
    south_m: float
    east_m: float
    north_m: float
    tolerance_m: float = SETBACK_TOLERANCE_M

    def __post_init__(self) -> None:
        for value, field in (
            (self.west_m, "west_m"), (self.south_m, "south_m"),
            (self.east_m, "east_m"), (self.north_m, "north_m"),
        ):
            _finite_number(value, field)
        if (
            isinstance(self.tolerance_m, bool)
            or not isinstance(self.tolerance_m, (int, float))
            or not math.isfinite(self.tolerance_m)
            or self.tolerance_m < 0
        ):
            raise ValueError("tolerance_m must be finite and non-negative")
        if self.west_m >= self.east_m or self.south_m >= self.north_m:
            raise ValueError("buildable area bounds must have positive width and depth")

    def contains(self, footprint: Footprint) -> bool:
        """Return whether every footprint corner lies in the closed area."""
        if not isinstance(footprint, Footprint):
            raise ValueError("footprint must be Footprint")
        # Keep the historical axis-aligned comparisons exact. Tolerance is
        # needed only when trigonometry can perturb a rotated corner.
        tolerance = 0.0 if footprint.rotation_degrees == 0.0 else self.tolerance_m
        return all(
            self.west_m - tolerance <= point.x_m <= self.east_m + tolerance
            and self.south_m - tolerance <= point.y_m <= self.north_m + tolerance
            for point in footprint.corners()
        )


def _finite_positive(value: Any, field: str) -> float:
    result = _finite_number(value, field)
    if result <= 0:
        raise ValueError(f"{field} must be positive")
    return result


def _finite_nonnegative(value: Any, field: str) -> float:
    result = _finite_number(value, field)
    if result < 0:
        raise ValueError(f"{field} must be non-negative")
    return result


def _rotation(value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError("rotation_degrees must be a number")
    result = float(value)
    if not math.isfinite(result) or not 0 <= result < 360:
        raise ValueError("rotation_degrees must be finite and in [0, 360)")
    return result


def _finite_number(value: Any, field: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{field} must be a number")
    result = float(value)
    if not math.isfinite(result):
        raise ValueError(f"{field} must be finite")
    return result


def _rotation_components(degrees: float) -> tuple[float, float]:
    cardinal = {
        0.0: (1.0, 0.0),
        90.0: (0.0, 1.0),
        180.0: (-1.0, 0.0),
        270.0: (0.0, -1.0),
    }
    if degrees in cardinal:
        return cardinal[degrees]
    radians = math.radians(degrees)
    return math.cos(radians), math.sin(radians)
