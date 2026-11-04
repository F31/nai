import type {PublicMediaVisual,PublicOutputStatus} from '@nai/contracts';
const sha=/^[0-9a-f]{64}$/;
export function validPublicVisual(v:PublicMediaVisual|undefined,name:string,license:string){return !!v&&v.artifactId==='d074f2a8-a618-4b43-9e24-679bf43a0001'&&sha.test(v.packageSha256)&&sha.test(v.licenseSha256)&&v.width===512&&v.height===288&&v.fps===30&&name==='内置简洁2D形象'&&license==='MIT';}
export const validOutputId=(id:string)=>/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(id);
export function validBoundOutput(s:PublicOutputStatus|undefined){return !!s&&validOutputId(s.outputId)&&s.protocolMajor===1&&s.windowOpen&&s.standby&&!s.liveReady&&!s.audioActive&&!s.terminal&&s.reason===1&&/^\d+$/.test(s.seq)&&BigInt(s.seq)>0n&&/^\d+$/.test(s.challenge)&&BigInt(s.challenge)>0n&&/^\d+$/.test(s.standbyFrames)&&BigInt(s.standbyFrames)>0n&&validPublicVisual(s.visual,s.visualName,s.visualLicense);}
