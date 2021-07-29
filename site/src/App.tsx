import { Box, Flex, Grid, Heading, Image, Link, Text, useColorMode } from '@chakra-ui/react'
import React, { useState } from 'react'
import { useQuery } from 'react-query'
import weverse from "./weverse.png"
import formatRelative from "date-fns/formatRelative"

type Data = {
  provider_name: string,
  url: string,
  response_code: number | null,
  response_delay: number | null,
  date: string
  media: Array<{
    media_url: string,
    page_url: string
  }>
}

const providerMappings = {
  "pinterest.board_feed": "https://e7.pngegg.com/pngimages/854/804/png-clipart-pinterest-logo-square-pinterest-icon-icons-logos-emojis-social-media-icons-thumbnail.png",
  "weverse.artist_feed": weverse
}

function App() {
  const { data } = useQuery<Data[]>('requests', () => fetch("/api/requests").then(r => r.json()))
  const a = useColorMode()
  console.log(data)
  return (
    <Flex flexFlow="column" alignItems="center" px={6} my={8} justifyContent="center">
      <Heading mb={6}>
        Latest Updates
      </Heading>
      <Flex as="main" maxWidth="1200px" flex={1} width="100%" flexFlow="column">
        <Grid gap={8}>
          {data?.map(scrape => {
            let hasMedia = scrape.media.length > 0;
            return (
              <Flex flexDirection="column" p={3} borderRadius="sm" borderWidth={1} overflow="hidden" borderColor={hasMedia ? "gray.700" : "gray.900"}
                background="gray.900"
              >
                <Flex justifyContent="space-between">
                  <Flex alignItems="center" >
                    <Image src={providerMappings[scrape.provider_name as keyof typeof providerMappings] as string} htmlWidth={"20px"} mr={4} borderRadius="full" />
                    <Text as="pre" color={hasMedia ? "gray.100" : "gray.600"} fontSize={hasMedia ? "md" : "sm"}>{scrape.url}</Text>
                  </Flex>
                  <Box as='time' fontSize="md" color={hasMedia ? "gray.500" : "gray.600"}>{formatRelative(new Date(scrape.date), new Date())}</Box>
                </Flex>
                {hasMedia &&
                  <>
                    <Flex flexFlow="row wrap" mt={4}>
                      {scrape.media.map(m => (
                        <Link href={m.page_url} target="_blank" rel="external noopener noreferrer">
                          <Image src={m.media_url} maxHeight="120px" mr={2} mb={2} />
                        </Link>
                      ))}
                    </Flex></>
                }
              </Flex>
            )
          })}
        </Grid>
        {/* <Text as="code">
          {JSON.stringify(data, null, 2)}
        </Text> */}
      </Flex>
    </Flex >
  )
}

export default App
