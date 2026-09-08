// Build-time entry only. The shipped bundle runs inside QuickJS, without Node or a browser.
import { mathjax } from 'mathjax-full/js/mathjax.js';
import { TeX } from 'mathjax-full/js/input/tex.js';
import { SVG } from 'mathjax-full/js/output/svg.js';
import { liteAdaptor } from 'mathjax-full/js/adaptors/liteAdaptor.js';
import { RegisterHTMLHandler } from 'mathjax-full/js/handlers/html.js';
import { AllPackages } from 'mathjax-full/js/input/tex/AllPackages.js';
const adaptor = liteAdaptor();
RegisterHTMLHandler(adaptor);
const document = mathjax.document('', {
  InputJax: new TeX({ packages: AllPackages.filter(p => p !== 'autoload' && p !== 'require'), maxBuffer: 100000 }),
  OutputJax: new SVG({ fontCache: 'none' })
});
globalThis.huiMathSvg = (source, display) => {
  const container = document.convert(source, { display, em: 16, ex: 8, containerWidth: 1280 });
  const svg = adaptor.firstChild(container);
  const width = parseFloat(adaptor.getAttribute(svg, 'width'));
  const height = parseFloat(adaptor.getAttribute(svg, 'height'));
  const align = parseFloat((adaptor.getAttribute(svg, 'style') || '').replace('vertical-align:', '')) || 0;
  adaptor.setAttribute(svg, 'width', String(width * 8));
  adaptor.setAttribute(svg, 'height', String(height * 8));
  adaptor.removeAttribute(svg, 'style');
  return JSON.stringify({ svg: adaptor.outerHTML(svg), width: width / 2, height: height / 2, align: align / 2 });
};
