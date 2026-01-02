import { Text } from "@mantine/core";
import { useMemo, type CSSProperties } from "react";
import { isDiffTrivial, type TextDiffChunk } from "../lib/textDiff";

export function InlineTextDiff({ chunks }: { chunks: TextDiffChunk[] }) {
  const rendered = useMemo(() => {
    if (chunks.length === 0 || isDiffTrivial(chunks)) return null;

    return chunks.map((c, idx) => {
      const style: CSSProperties = {
        borderRadius: 3,
        padding: c.added || c.removed ? "0 2px" : undefined,
        backgroundColor: c.added
          ? "rgba(34, 197, 94, 0.18)"
          : c.removed
          ? "rgba(239, 68, 68, 0.18)"
          : undefined,
        outline: c.added
          ? "1px solid rgba(34, 197, 94, 0.25)"
          : c.removed
          ? "1px solid rgba(239, 68, 68, 0.25)"
          : undefined,
      };

      return (
        <span key={idx} style={style}>
          {c.value}
        </span>
      );
    });
  }, [chunks]);

  if (!rendered) return null;

  return (
    <Text
      size="sm"
      component="div"
      style={{
        lineHeight: 1.5,
        whiteSpace: "pre-wrap",
        wordBreak: "break-word",
      }}
    >
      {rendered}
    </Text>
  );
}
