import { Center, Loader, Text } from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { booksApi } from "@/api/books";
import { ReaderRouter } from "@/components/reader";

/**
 * Reader page component.
 *
 * Loads book data and passes it to the appropriate reader component.
 * Handles navigation back to book detail page on close.
 */
export function Reader() {
  const { bookId } = useParams<{ bookId: string }>();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();

  // Extract page query parameter (e.g., ?page=17) for comics/PDFs
  const pageParam = searchParams.get("page");
  const startPage = pageParam ? Number.parseInt(pageParam, 10) : undefined;

  // Extract percent query parameter (e.g., ?percent=45) for EPUBs
  // Accepts 0-100 and converts to 0.0-1.0
  const percentParam = searchParams.get("percent");
  const startPercent = percentParam
    ? Number.parseFloat(percentParam) / 100
    : undefined;

  // Extract incognito parameter (e.g., ?incognito=true) for reading without progress tracking
  const incognito = searchParams.get("incognito") === "true";

  // Fetch book details (includes effective reading direction from series/library)
  const {
    data: bookDetail,
    isLoading: bookLoading,
    error: bookError,
  } = useQuery({
    queryKey: ["book", bookId],
    queryFn: () => booksApi.getDetail(bookId as string),
    enabled: !!bookId,
  });

  const handleClose = () => {
    // Navigate back to book detail page
    if (bookId) {
      navigate(`/books/${bookId}`);
    } else {
      navigate(-1);
    }
  };

  // Loading state
  if (bookLoading) {
    return (
      <Center
        style={{ width: "100vw", height: "100vh", backgroundColor: "#000" }}
      >
        <Loader size="lg" color="gray" />
      </Center>
    );
  }

  // Error state
  if (bookError || !bookDetail) {
    return (
      <Center
        style={{ width: "100vw", height: "100vh", backgroundColor: "#000" }}
      >
        <Text c="red">
          {bookError instanceof Error
            ? bookError.message
            : "Failed to load book"}
        </Text>
      </Center>
    );
  }

  const { book } = bookDetail;

  return (
    <ReaderRouter
      bookId={book.id}
      seriesId={book.seriesId}
      title={book.title}
      totalPages={book.pageCount}
      format={book.fileFormat}
      fileSize={book.fileSize}
      readingDirection={book.readingDirection ?? null}
      analyzed={book.analyzed}
      startPage={startPage}
      startPercent={startPercent}
      incognito={incognito}
      onClose={handleClose}
    />
  );
}
