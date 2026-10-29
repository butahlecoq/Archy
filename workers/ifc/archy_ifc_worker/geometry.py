"""Task02 rectangle/L measurements and local profile coordinates."""


def footprint(variant):
    points = variant["footprint_xy_m"]
    ox, oy = variant["parameters"]["origin_xy_m"]
    local = [(x - ox, y - oy) for x, y in points]
    area = abs(sum(a[0] * b[1] - b[0] * a[1]
                   for a, b in zip(local, local[1:]))) / 2
    return local, area
