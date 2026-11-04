"""Scale a research composite box onto a smaller frame.

The 256 latent stays on the original crop. This only moves the rectangle
where the decoded face is pasted. It does not authorize that frame as
public media.
"""


def scale_box(box, src_w, src_h, dst_w, dst_h):
    if any(type(value) is not int or value <= 0 for value in (src_w, src_h, dst_w, dst_h)):
        raise ValueError("invalid frame size")
    if type(box) is not list or len(box) != 4 or any(type(value) is not int for value in box):
        raise ValueError("invalid box")
    left, top, right, bottom = box
    if not (0 <= left < right <= src_w and 0 <= top < bottom <= src_h):
        raise ValueError("box outside the source frame")

    def axis(value, src, dst):
        return int(round(value * dst / src))

    scaled_left = min(max(axis(left, src_w, dst_w), 0), dst_w - 2)
    scaled_top = min(max(axis(top, src_h, dst_h), 0), dst_h - 2)
    scaled_right = min(max(axis(right, src_w, dst_w), scaled_left + 1), dst_w)
    scaled_bottom = min(max(axis(bottom, src_h, dst_h), scaled_top + 1), dst_h)
    return [scaled_left, scaled_top, scaled_right, scaled_bottom]
