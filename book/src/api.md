# API

Sorock provides only simple APIs.

## Create(Key, Value)

Create a key-value pair in the storage.
Key shouldn't be reused for any two different values.
Typically, the key is generated from the value using hash function like SHA1.

## Read(Key)

Read the value from the storage.

## Delete(Key)

Delete the key-value pair.

## AddNode(URI, Capacity)

Add a node in the cluster.
Regarding the Capacity, recommendation is setting x if the node has xTB local storage.

## RemoveNode(URI)

Remove a node from the cluster.