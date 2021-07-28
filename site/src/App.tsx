import { Box, Flex, Heading, useColorMode } from '@chakra-ui/react'
import React, { useState } from 'react'

function App() {
  const [count, setCount] = useState(0)
  const a = useColorMode()
  return (
    <Flex flexFlow="column" alignItems="center" px={6} my={8} justifyContent="center">
      <Box as="header">

      </Box>
      <Flex as="main" maxWidth="1200px" flex={1} width="100%">
        <Heading>
          Latest Requests
        </Heading>
      </Flex>
    </Flex>
  )
}

export default App
