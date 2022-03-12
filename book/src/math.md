# Math

## How many pieces are moved on cluster change?

In computing N holder nodes for a key,
computing the holder node for each (key, index) pair independently
is a local-optimal solution.
In this case, it is guaranteed that only one piece is moved
per key for any one cluster change but occasionally happen to 
place all pieces in one node which lose the redundancy we hope to have:
for (N,K) erasure coding we should be allowed to lose N-K pieces.

So choose N independent holders and let's estimate how many pieces
are moved per cluster change.

Suppose the placement before cluster change is S1,S2,S3,S4 (n=4)
which is computed by consistent manner
(Sorock uses ASURA but you can use consistent-hashing or whatever that functions the same)
and There is an equal possibility of losing each server.
If we remove S2 for example and the next placement is S1,S3,S4,S5 we need to move 3 pieces
(S2->S3,S3->S4,S4->S5). If we remove S4 only one piece is moved.

The average moves will be

\\[ \frac{1}{N} \sum_{i=1}^{N-1} i = \frac{N-1}{2} \\]

If there are C nodes in the cluster, the possibility of choose either one in N nodes is

\\[ \frac{N}{C} \\]

then the expectation number of moves per key will be

\\[ \frac{N(N-1)}{2C} \\]

Because the possibility that one node out of C nodes went down
in a certain period
is proportional to C

\\[ pC \\]

So the expectation number of moves per key per time will be


\\[ \frac{pN(N-1)}{2} \\]

## How many random numbers are need to compute N holders?

Suppose all nodes have the same capacity.
In ASURA, we need 1 random number to choose the first node.
Because we need to choose the second node other than the choosen node,
the expection number for the second choice will be

\\[ \frac{C}{C-1} \\]

So the expection number to compute the all N holders in general will be 

\\[ \sum_{i=0}^{N-1} \frac{C}{C-i} = \sum_{i=0}^{N-1} ( 1 + \frac{i}{C-i} ) = N + \sum_{i=0}^{N-1} \frac{i}{C-i} \\]

This means when C is large enough (100~) the cost of computing N holders is decreasing to only N (one random number per holder which is super fast). Since the holder computation is frequently executed in the implementation and erasure-coding storage uses a lot of computational resource, the cost should be lower as possible. This is why I choose to use ASURA.