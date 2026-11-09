import React from "react";
import ReactMarkdown, { Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeRaw from "rehype-raw";
import rehypeSanitize from "rehype-sanitize";
import { openExternal } from "../../lib/tauri";

/**
 * Project descriptions come from strangers on the internet. They are Markdown with
 * some HTML mixed in, so the HTML is parsed and then sanitized: no scripts, iframes,
 * styles or event handlers survive, only plain formatting, links and images.
 */
const components: Components = {
  h1: ({ children }) => <h3 className="text-xl font-extrabold mt-6 mb-2 first:mt-0">{children}</h3>,
  h2: ({ children }) => <h4 className="text-lg font-bold mt-5 mb-2 first:mt-0">{children}</h4>,
  h3: ({ children }) => <h5 className="font-bold mt-4 mb-1.5 first:mt-0">{children}</h5>,
  h4: ({ children }) => <h5 className="font-bold mt-4 mb-1.5 first:mt-0">{children}</h5>,
  h5: ({ children }) => <h5 className="font-bold mt-3 mb-1 first:mt-0">{children}</h5>,
  h6: ({ children }) => <h5 className="font-bold mt-3 mb-1 first:mt-0">{children}</h5>,
  p: ({ children }) => <p className="my-2.5 leading-relaxed">{children}</p>,
  ul: ({ children }) => <ul className="my-2.5 pl-6 list-disc flex flex-col gap-1">{children}</ul>,
  ol: ({ children }) => <ol className="my-2.5 pl-6 list-decimal flex flex-col gap-1">{children}</ol>,
  blockquote: ({ children }) => (
    <blockquote className="my-3 pl-4 border-l-4 border-line text-soft">{children}</blockquote>
  ),
  hr: () => <hr className="my-5 border-line" />,
  a: ({ href, children }) => (
    <a
      href={href}
      onClick={(e) => {
        // Never navigate the app window itself; links open in the browser.
        e.preventDefault();
        if (href) openExternal(href).catch(() => {});
      }}
      className="text-info font-bold hover:underline cursor-pointer"
    >
      {children}
    </a>
  ),
  img: ({ src, alt, width, height }) => (
    <img
      src={src}
      alt={alt ?? ""}
      width={width}
      height={height}
      loading="lazy"
      referrerPolicy="no-referrer"
      className="inline-block max-w-full h-auto rounded-xl my-1"
    />
  ),
  code: ({ children, className }) =>
    className ? (
      <code className={`${className} font-mono text-sm`}>{children}</code>
    ) : (
      <code className="font-mono text-[0.9em] bg-raised rounded-md px-1.5 py-0.5">{children}</code>
    ),
  pre: ({ children }) => (
    <pre className="my-3 p-4 rounded-xl bg-raised overflow-x-auto select-text">{children}</pre>
  ),
  table: ({ children }) => (
    <div className="my-3 overflow-x-auto">
      <table className="w-full text-sm border-collapse">{children}</table>
    </div>
  ),
  th: ({ children }) => (
    <th className="text-left font-bold px-3 py-2 border-b border-line">{children}</th>
  ),
  td: ({ children }) => <td className="px-3 py-2 border-b border-line/60">{children}</td>,
  details: ({ children }) => <details className="my-3 rounded-xl bg-raised/60 px-4 py-2">{children}</details>,
  summary: ({ children }) => <summary className="font-bold cursor-pointer py-1">{children}</summary>,
};

export const Markdown: React.FC<{ children: string }> = ({ children }) => (
  <div className="select-text break-words">
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      rehypePlugins={[rehypeRaw, rehypeSanitize]}
      components={components}
    >
      {children}
    </ReactMarkdown>
  </div>
);
