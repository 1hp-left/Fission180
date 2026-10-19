import { draw, drawCircle, drawRectangle, type Drawing, type Point2D } from 'replicad';
import { type CadDocument, type Profile, resolveScalar } from './document.js';

export const LENGTH_TOLERANCE = 1e-5;
type Point = [number, number];
const cross = (a: Point, b: Point, c: Point) => (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
const distance = (a: Point, b: Point) => Math.hypot(a[0] - b[0], a[1] - b[1]);

function onSegment(a: Point, b: Point, p: Point): boolean {
  return Math.abs(cross(a, b, p)) <= LENGTH_TOLERANCE * distance(a, b)
    && p[0] >= Math.min(a[0], b[0]) - LENGTH_TOLERANCE && p[0] <= Math.max(a[0], b[0]) + LENGTH_TOLERANCE
    && p[1] >= Math.min(a[1], b[1]) - LENGTH_TOLERANCE && p[1] <= Math.max(a[1], b[1]) + LENGTH_TOLERANCE;
}

function intersects(a: Point, b: Point, c: Point, d: Point): boolean {
  const c1 = cross(a, b, c), c2 = cross(a, b, d), c3 = cross(c, d, a), c4 = cross(c, d, b);
  const opposite = (x: number, y: number) => (x > 0 && y < 0) || (x < 0 && y > 0);
  return (opposite(c1, c2) && opposite(c3, c4))
    || onSegment(a, b, c) || onSegment(a, b, d) || onSegment(c, d, a) || onSegment(c, d, b);
}

/** Polygons are implicitly closed. Reject crossings, touches, and backtracking. */
export function validatePolygon(points: Point[]): void {
  let twiceArea = 0;
  for (let i = 0; i < points.length; i++) {
    const a = points[i]!, b = points[(i + 1) % points.length]!, c = points[(i + 2) % points.length]!;
    if (distance(a, b) <= LENGTH_TOLERANCE) throw new Error('Polygon has a zero-length edge; omit the repeated closing vertex');
    if (onSegment(a, b, c) || onSegment(b, c, a)) throw new Error('Polygon has overlapping adjacent edges');
    twiceArea += a[0] * b[1] - b[0] * a[1];
    for (let j = i + 2; j < points.length; j++) {
      if (i === 0 && j === points.length - 1) continue;
      if (intersects(a, b, points[j]!, points[(j + 1) % points.length]!)) throw new Error('Polygon is self-intersecting or self-touching');
    }
  }
  if (Math.abs(twiceArea) <= LENGTH_TOLERANCE ** 2) throw new Error('Polygon has zero area');
}

export function createProfile(profile: Profile, parameters: CadDocument['parameters']): Drawing {
  const value = (v: Parameters<typeof resolveScalar>[0]) => resolveScalar(v, parameters);
  const positive = (dimension: number, name: string) => {
    if (dimension <= LENGTH_TOLERANCE) throw new Error(`${name} must exceed ${LENGTH_TOLERANCE} mm`);
    return dimension;
  };
  if (profile.type === 'polygon') {
    const points = profile.points.map(([x, y]): Point => [value(x), value(y)]);
    validatePolygon(points);
    const pen = draw(points[0]!);
    for (const point of points.slice(1)) pen.lineTo(point);
    return pen.close();
  }
  const center: Point2D = [value(profile.center[0]), value(profile.center[1])];
  if (profile.type === 'circle') return drawCircle(positive(value(profile.radius), 'Radius')).translate(center);
  return drawRectangle(positive(value(profile.width), 'Width'), positive(value(profile.height), 'Height'))
    .rotate(value(profile.rotation)).translate(center);
}
