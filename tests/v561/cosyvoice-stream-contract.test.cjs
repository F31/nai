const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const Ajv = require('ajv/dist/2020.js').default;
const SwaggerParser = require('@apidevtools/swagger-parser');

test('CosyVoice config validates canonical deployment and rejects unsafe/ambiguous input', () => {
  const schema = JSON.parse(fs.readFileSync('schemas/config/cosyvoice-stream.schema.json', 'utf8'));
  const validate = new Ajv({strict:true}).compile(schema);
  const config = {endpoint:'http://127.0.0.1:50000/inference_zero_shot', reference_file:'E:/authorized/reference.wav',
    prompt_text:'You are a helpful assistant.<|endofprompt|>参考文本', deadline_ms:45000, max_samples:480000};
  assert.equal(validate(config), true, JSON.stringify(validate.errors));
  assert.equal(validate({...config,lookahead:{max_sentence_chars:80,max_sentence_samples:144000,max_sentences:16,idle_wait_ms:4000}}),true);
  for (const lookahead of [{max_sentence_chars:15},{max_sentence_samples:240001},{max_sentences:33},{idle_wait_ms:10001},{prefetch_slots:2}]) {
    assert.equal(validate({...config,lookahead}),false,JSON.stringify(lookahead));
  }
  for (const change of [{endpoint:'http://192.168.0.2:50000/inference_zero_shot'}, {prompt_text:'missing marker'},
    {deadline_ms:45001}, {max_samples:480001}, {apiKey:'not part of local protocol'}]) {
    assert.equal(validate({...config,...change}), false, JSON.stringify(change));
  }
});

test('CosyVoice HTTP source declares raw PCM streaming and busy without retry', async () => {
  const api = await SwaggerParser.validate('schemas/openapi/cosyvoice-stream.yaml');
  const operation = api.paths['/inference_zero_shot'].post;
  assert.equal(operation.requestBody.content['multipart/form-data'].schema.properties.stream.enum[0], true);
  assert.ok(operation.responses['200'].content['application/octet-stream']);
  assert.ok(operation.responses['409']);
  assert.equal(operation.responses['200'].headers['X-Audio-Encoding'].schema.enum[0], 'pcm_s16le');
  assert.ok(operation.responses['200'].headers['X-Nai-Request']);
  assert.ok(operation.responses['200'].headers['X-Nai-Instance']);
  const receipt = api.paths['/nai/tts/status/{request_id}'].get;
  assert.ok(receipt.responses['404']);
  assert.equal(api.components.schemas.Completion.additionalProperties, false);
  assert.equal(api.components.schemas.Completion.properties.samples.maximum, 480000);
});
