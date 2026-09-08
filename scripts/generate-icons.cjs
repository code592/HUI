#!/usr/bin/env node
// Deterministic native icon assets. Install sharp 0.35.4 or set NODE_PATH to an existing installation.
const fs = require('node:fs');
const path = require('node:path');
const sharp = require('sharp');
const root = path.resolve(__dirname, '..');
const master = fs.readFileSync(path.join(root, 'assets/brand/hui.svg'), 'utf8');
const mark = master.match(/<g id="mark"[\s\S]*?<\/g>/)[0];
const svg = (body) => `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">${body}</svg>`;
const full = svg(`<rect width="1024" height="1024" fill="#477AD9"/>${mark}`);
const mac = svg(`<g transform="translate(80 80) scale(.84375)">${master.match(/<rect[^>]+\/>/)[0]}${mark}</g>`);
const foreground = svg(`<g transform="translate(51.2 51.2) scale(.9)">${mark}</g>`);
const mono = svg(mark.replace('fill="#FFFFFF"', 'fill="#477AD9"'));
function write(name, data) { const p = path.join(root, name); fs.mkdirSync(path.dirname(p), { recursive: true }); fs.writeFileSync(p, data); }
function json(name, data) { write(name, JSON.stringify(data, null, 2) + '\n'); }
async function png(source, size, opaque = false) {
  let image = sharp(Buffer.from(source), { density: 144 }).resize(size, size);
  if (opaque) image = image.flatten({ background: '#477AD9' }).removeAlpha();
  return image.png().toBuffer();
}
function ico(images) {
  const header = Buffer.alloc(6); header.writeUInt16LE(1, 2); header.writeUInt16LE(images.length, 4);
  let offset = 6 + images.length * 16;
  const entries = images.map(({ size, data }) => { const b = Buffer.alloc(16); b[0] = b[1] = size === 256 ? 0 : size; b.writeUInt16LE(1, 4); b.writeUInt16LE(32, 6); b.writeUInt32LE(data.length, 8); b.writeUInt32LE(offset, 12); offset += data.length; return b; });
  return Buffer.concat([header, ...entries, ...images.map(i => i.data)]);
}
async function main() {
  write('assets/brand/hui-mark.svg', mono);
  write('assets/brand/hui-square.svg', full);
  write('assets/brand/hui-foreground.svg', foreground);
  write('assets/brand/hui.png', await png(master, 1024));
  write('assets/brand/hui-512.png', await png(master, 512));
  write('assets/brand/hui-256.png', await png(master, 256));
  const images = await Promise.all([16,24,32,48,64,128,256].map(async size => ({size, data: await png(master, size)})));
  write('packaging/windows/hui.ico', ico(images));
  const types = [['icp4',16],['icp5',32],['icp6',64],['ic07',128],['ic08',256],['ic09',512],['ic10',1024],['ic11',32],['ic12',64],['ic13',256],['ic14',512]];
  const chunks = await Promise.all(types.map(async ([type, size]) => { const data = await png(mac,size); const header = Buffer.alloc(8); header.write(type); header.writeUInt32BE(data.length+8,4); return Buffer.concat([header,data]); }));
  const icns = Buffer.alloc(8); icns.write('icns'); icns.writeUInt32BE(8+chunks.reduce((sum,b)=>sum+b.length,0),4);
  write('packaging/macos/HUI.icns', Buffer.concat([icns,...chunks]));
  for (const size of [16,24,32,48,64,128,256,512]) write(`packaging/linux/icons/hicolor/${size}x${size}/apps/hui.png`, await png(master,size));
  write('packaging/linux/icons/hicolor/scalable/apps/hui.svg', master);
  const ios = [];
  for (const [idiom, sizes] of [['iphone',[[20,[2,3]],[29,[2,3]],[40,[2,3]],[60,[2,3]]]], ['ipad',[[20,[1,2]],[29,[1,2]],[40,[1,2]],[76,[1,2]],[83.5,[2]]]]]) {
    for (const [size, scales] of sizes) for (const scale of scales) {
      const filename = `icon-${size*scale}.png`;
      write(`packaging/ios/Assets.xcassets/AppIcon.appiconset/${filename}`,await png(full,size*scale,true));
      ios.push({idiom,size:`${size}x${size}`,scale:`${scale}x`,filename});
    }
  }
  write('packaging/ios/Assets.xcassets/AppIcon.appiconset/icon-1024.png',await png(full,1024,true));
  ios.push({idiom:'ios-marketing',size:'1024x1024',scale:'1x',filename:'icon-1024.png'});
  json('packaging/ios/Assets.xcassets/AppIcon.appiconset/Contents.json',{images:ios,info:{author:'xcode',version:1}});
  json('packaging/ios/Assets.xcassets/Contents.json',{info:{author:'xcode',version:1}});
  const res = 'packaging/android/res';
  for (const [density,size] of [['mdpi',48],['hdpi',72],['xhdpi',96],['xxhdpi',144],['xxxhdpi',192]]) {
    write(`${res}/mipmap-${density}/ic_launcher.png`,await png(master,size));
    write(`${res}/mipmap-${density}/ic_launcher_round.png`,await png(svg(`<circle cx="512" cy="512" r="512" fill="#477AD9"/>${foreground.match(/<g[\s\S]*<\/g>/)[0]}`),size));
  }
  const paths = [...mark.matchAll(/<path\s+d="([^"]+)"/g)].map(m=>m[1]);
  const vector = (color) => `<vector xmlns:android="http://schemas.android.com/apk/res/android" android:width="108dp" android:height="108dp" android:viewportWidth="1024" android:viewportHeight="1024"><group android:pivotX="512" android:pivotY="512" android:scaleX="0.9" android:scaleY="0.9">${paths.map(d=>`<path android:fillColor="${color}" android:pathData="${d}"/>`).join('')}</group></vector>\n`;
  write(`${res}/drawable/ic_launcher_foreground.xml`,vector('#FFFFFF'));
  write(`${res}/drawable/ic_launcher_monochrome.xml`,vector('#FFFFFF'));
  write(`${res}/values/icon_colors.xml`,'<resources><color name="hui_icon_background">#477AD9</color></resources>\n');
  for (const version of [26,33]) for (const name of ['ic_launcher','ic_launcher_round']) write(`${res}/mipmap-anydpi-v${version}/${name}.xml`,`<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android"><background android:drawable="@color/hui_icon_background"/><foreground android:drawable="@drawable/ic_launcher_foreground"/>${version===33?'<monochrome android:drawable="@drawable/ic_launcher_monochrome"/>':''}</adaptive-icon>\n`);
  // A review sheet showing small sizes and platform masks; not shipped in the application.
  const cells = [{source:master,x:96,y:88,size:256},{source:mac,x:422,y:88,size:256},{source:full,x:748,y:88,size:256}];
  const layers = await Promise.all(cells.map(async c=>({input:await png(c.source,c.size),left:c.x,top:c.y})));
  for (const [i,size] of [16,24,32,48,64,96].entries()) layers.push({input:await png(master,size),left:108+i*150,top:450+Math.floor((96-size)/2)});
  write('docs/screenshots/logo-preview.png',await sharp({create:{width:1100,height:630,channels:4,background:'#f4f5f7'}}).composite(layers).png().toBuffer());
  console.log('Generated HUI icons for macOS, Windows, Linux, Android and iOS.');
}
main().catch(e=>{console.error(e);process.exitCode=1;});
