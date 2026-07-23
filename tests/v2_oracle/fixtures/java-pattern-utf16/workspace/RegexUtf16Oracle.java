import java.util.regex.Pattern;

public final class RegexUtf16Oracle {
  private static final String ASCII_HIGH_SURROGATE_ESCAPE =
      new String(new char[] {0x005c, 0x0075, 0x0044, 0x0038, 0x0030, 0x0030});
  private static final String NUL = new String(new char[] {0x0000});
  private static final String UNPAIRED_HIGH_SURROGATE = new String(new char[] {(char) 0xd800});

  public static void main(String[] args) {
    Pattern pattern = Pattern.compile(ASCII_HIGH_SURROGATE_ESCAPE);
    boolean nulFind = pattern.matcher(NUL).find();
    boolean unpairedHighSurrogateFind = pattern.matcher(UNPAIRED_HIGH_SURROGATE).find();

    if (nulFind || !unpairedHighSurrogateFind) {
      throw new AssertionError(
          "unexpected Pattern.find results: nul="
              + nulFind
              + ", unpaired_high_surrogate="
              + unpairedHighSurrogateFind);
    }

    System.err.println("java.runtime.version=" + System.getProperty("java.runtime.version"));
    System.out.println("pattern=ascii_high_surrogate_escape subject=nul find=" + nulFind);
    System.out.println(
        "pattern=ascii_high_surrogate_escape subject=unpaired_high_surrogate find="
            + unpairedHighSurrogateFind);
  }
}
