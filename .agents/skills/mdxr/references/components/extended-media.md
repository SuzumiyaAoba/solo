# Media components

All data and options attributes contain literal JSON strings. No MDX expressions are needed. HTML includes interactive controls; ASCII retains static data.

## ImageGallery

See the example and `mdxr catalog` for the public attributes.

```mdx
<ImageGallery data='[{"src":"assets/before.svg","alt":"Before layout","caption":"Before"},{"src":"assets/after.svg","alt":"After layout","caption":"After"}]' />
```

## AnnotatedImage

See the example and `mdxr catalog` for the public attributes.

```mdx
<AnnotatedImage
  data='[{"x":15,"y":25,"width":30,"height":35,"label":"Navigation","note":"Persistent navigation area."}]'
  src="assets/after.svg"
  alt="Example navigation layout"
/>
```

## ImageCompare

See the example and `mdxr catalog` for the public attributes.

```mdx
<ImageCompare
  before="assets/before.svg"
  after="assets/after.svg"
  beforeAlt="Before layout"
  afterAlt="After layout"
/>
```

## Video

See the example and `mdxr catalog` for the public attributes.

```mdx
<Video
  src="assets/example.webm"
  captions="assets/captions.vtt"
  poster="assets/before.svg"
/>
```

## AudioTranscript

See the example and `mdxr catalog` for the public attributes.

```mdx
<AudioTranscript
  data='[{"time":0,"text":"An example tone starts here."},{"time":0.5,"text":"The example ends."}]'
  src="assets/tone.wav"
  captions="assets/captions.vtt"
/>
```

## PdfPreview

See the example and `mdxr catalog` for the public attributes.

```mdx
<PdfPreview
  src="assets/example.pdf"
  title="Example report"
  page="1"
  height="300"
/>
```

## PrintLayout

See the example and `mdxr catalog` for the public attributes.

```mdx
<PrintLayout columns='2'>

First page.

<PageBreak />

Second page.

</PrintLayout>
```

## TableOfFigures

See the example and `mdxr catalog` for the public attributes.

```mdx
<TableOfFigures />

<Figure src='assets/after.svg' id='listed-figure' caption='Listed figure'>

Figure content.

</Figure>
```

## NumberedEquation

See the example and `mdxr catalog` for the public attributes.

```mdx
<NumberedEquation id='velocity'>

$$
v = d / t
$$

</NumberedEquation>

See <CrossRef target="velocity" />.
```

## Theorem

See the example and `mdxr catalog` for the public attributes.

```mdx
<Theorem id='even-sum' title='Even sums'>

The sum of two even integers is even.

</Theorem>

<Proof id='even-proof'>

Write the integers as $2a$ and $2b$. Their sum is $2(a+b)$.

</Proof>
```
