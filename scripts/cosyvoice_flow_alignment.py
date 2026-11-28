"""Explicit Flow-only reference candidate; full LLM/voice and cache stay intact."""
import functools
import json
import math
import sys

FLOW_FIELDS=('flow_prompt_speech_token','flow_prompt_speech_token_len','prompt_speech_feat','prompt_speech_feat_len')


def settings(*, hop, ratio, frame_rate, feature_dim, minimum_seconds, max_bytes):
    if (any(type(x) is not int or x<1 for x in (hop,ratio,frame_rate,feature_dim,max_bytes))
            or max_bytes>16*1024*1024 or isinstance(minimum_seconds,bool)
            or not isinstance(minimum_seconds,(int,float)) or not math.isfinite(minimum_seconds)
            or not 0<minimum_seconds<=30):
        raise ValueError('FLOW_ALIGNMENT_CONFIG')


def align(conditions, *, hop, ratio, frame_rate, feature_dim, minimum_seconds, max_bytes, check):
    import torch
    check()
    settings(hop=hop,ratio=ratio,frame_rate=frame_rate,feature_dim=feature_dim,
        minimum_seconds=minimum_seconds,max_bytes=max_bytes)
    required=FLOW_FIELDS+('llm_prompt_speech_token','llm_prompt_speech_token_len','prompt_text','llm_embedding','flow_embedding')
    if type(conditions) is not dict or any(k not in conditions or not torch.is_tensor(conditions[k]) for k in required):
        raise ValueError('FLOW_ALIGNMENT_CLOSURE')
    token,length,feat,feat_length=(conditions[k] for k in FLOW_FIELDS)
    if (token.ndim!=2 or token.shape[0]!=1 or token.dtype not in (torch.int32,torch.int64)
            or feat.ndim!=3 or feat.shape[0]!=1 or feat.shape[2]!=feature_dim
            or not feat.is_floating_point() or feat.shape[1]!=token.shape[1]*ratio
            or any(t.numel()!=1 or t.dtype not in (torch.int32,torch.int64) for t in (length,feat_length))):
        raise ValueError('FLOW_ALIGNMENT_SHAPE')
    count=token.shape[1]
    if int(length.item())!=count or int(feat_length.item())!=count*ratio:
        raise ValueError('FLOW_ALIGNMENT_LENGTH')
    if sum(conditions[k].numel()*conditions[k].element_size() for k in FLOW_FIELDS)>max_bytes:
        raise ValueError('FLOW_ALIGNMENT_BYTES')
    target=count//hop*hop
    if target<math.ceil(frame_rate*minimum_seconds):raise ValueError('FLOW_ALIGNMENT_TOO_SHORT')
    check()
    result=dict(conditions)
    result.update(flow_prompt_speech_token=token[:,:target].detach().clone(),
        flow_prompt_speech_token_len=torch.full_like(length,target),
        prompt_speech_feat=feat[:,:target*ratio].detach().clone(),
        prompt_speech_feat_len=torch.full_like(feat_length,target*ratio))
    check()
    return result,dict(original_tokens=count,aligned_tokens=target,original_feature_frames=count*ratio,
        aligned_feature_frames=target*ratio,hop=hop,ratio=ratio,frame_rate=frame_rate,changed=target!=count)


def install(model, *, minimum_seconds, max_bytes, sink=None):
    frontend=model.frontend;core=model.model
    if getattr(frontend,'_nai_flow_alignment',None) is not None:raise ValueError('FLOW_ALIGNMENT_ALREADY_INSTALLED')
    config=dict(hop=core.token_hop_len,ratio=core.flow.token_mel_ratio,
        frame_rate=core.flow.input_frame_rate,feature_dim=core.flow.output_size,
        minimum_seconds=minimum_seconds,max_bytes=max_bytes)
    settings(**config)
    # Caller installs after the existing condition cache. Output is transformed,
    # never the cache entry. Existing request-gate cancel/drain remains unchanged.
    original=frontend.frontend_zero_shot
    def log(record):
        try:print('NAI_FLOW_ALIGNMENT '+json.dumps(record,separators=(',',':')),file=sys.stderr,flush=True)
        except OSError:pass
    emit=sink or log
    @functools.wraps(original)
    def wrapped(*args,**kwargs):
        result,metadata=align(original(*args,**kwargs),**config,check=lambda:None)
        try:emit(metadata)
        except Exception:pass
        return result
    frontend.frontend_zero_shot=wrapped
    frontend._nai_flow_alignment=True
    def close():
        if frontend.frontend_zero_shot is not wrapped:raise RuntimeError('FLOW_ALIGNMENT_OWNER')
        frontend.frontend_zero_shot=original;del frontend._nai_flow_alignment
    return close
