"""Explicit experimental normalization policy; changes checkpoint architecture."""
import math
import torch


def apply_normalization(model, policy):
    if policy == 'batch':
        return model
    if policy != 'group':
        raise ValueError('unknown normalization policy')
    for name, child in list(model.named_children()):
        if isinstance(child, torch.nn.BatchNorm2d):
            replacement = torch.nn.GroupNorm(math.gcd(32, child.num_features), child.num_features, eps=child.eps, affine=child.affine)
            if child.affine:
                with torch.no_grad():
                    replacement.weight.copy_(child.weight)
                    replacement.bias.copy_(child.bias)
            setattr(model, name, replacement)
        else:
            apply_normalization(child, policy)
    return model
