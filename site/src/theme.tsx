import { ChakraTheme, extendTheme } from "@chakra-ui/react";

export const theme: Partial<ChakraTheme> = extendTheme({
  config: {
    initialColorMode: "dark",
    useSystemColorMode: false
  }
})