// frontend/src/components/shared/Markdown.tsx
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeSanitize from 'rehype-sanitize'
import rehypeHighlight from 'rehype-highlight'
import { useThrottledValue } from '@/hooks/useThrottledValue'

interface MarkdownProps {
  content: string
  throttle?: number  // ms, default 80
}

export function Markdown({ content, throttle = 80 }: MarkdownProps) {
  const throttled = useThrottledValue(content, throttle)

  return (
    <div className="text-foreground leading-[1.5] prose prose-invert max-w-none prose-sm">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[
          [rehypeSanitize, {
            tagNames: ['p', 'div', 'span', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6',
              'ul', 'ol', 'li', 'blockquote', 'pre', 'code', 'table', 'thead',
              'tbody', 'tr', 'th', 'td', 'em', 'strong', 'del', 'a', 'br', 'hr',
              'input', 'details', 'summary'],
            attributes: {
              '*': ['className', 'id', 'data-*'],
              'a': ['href', 'title', 'target', 'rel'],
              'input': ['type', 'checked', 'disabled'],
              'code': ['className'],
              'pre': ['className'],
              'details': ['open'],
            },
            strip: ['script', 'iframe', 'object', 'embed', 'img', 'video', 'audio'],
          }],
          rehypeHighlight,
        ]}
      >
        {throttled || (content === '' ? '_Thinking..._' : '')}
      </ReactMarkdown>
    </div>
  )
}
