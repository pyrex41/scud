const pptxgen = require('pptxgenjs');
const path = require('path');
const html2pptx = require('/Users/reuben/gauntlet/scud/.claude/skills/pptx/scripts/html2pptx');

async function createPresentation() {
    const pptx = new pptxgen();
    pptx.layout = 'LAYOUT_16x9';
    pptx.author = 'SCUD';
    pptx.title = 'SCUD - Fast AI-Powered Task Management';
    pptx.subject = 'Introduction to SCUD workflow';

    const slidesDir = '/Users/reuben/gauntlet/scud/workspace/slides';
    const slides = [
        'slide1-title.html',
        'slide0-screenshot.html',
        'slide1b-install.html',
        'slide1c-taskmaster.html',
        'slide1d-scud-diff.html',
        'slide1e-scg-format.html',
        'slide6b-skills.html',
        'slide2-init.html',
        'slide3-parse.html',
        'slide4-expand.html',
        'slide5-waves.html',
        'slide5b-review.html',
        'slide6-workflow.html',
        'slide7-open-question.html'
    ];

    for (const slideFile of slides) {
        const htmlPath = path.join(slidesDir, slideFile);
        console.log(`Processing: ${slideFile}`);
        await html2pptx(htmlPath, pptx);
    }

    const outputPath = '/Users/reuben/gauntlet/scud/workspace/SCUD-Intro.pptx';
    await pptx.writeFile({ fileName: outputPath });
    console.log(`\nPresentation created: ${outputPath}`);
}

createPresentation().catch(console.error);
