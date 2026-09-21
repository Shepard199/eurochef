import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.Reference;
public class StalkThrottleCallAssemblyEvidence extends GhidraScript {
 private void dump(long start,long end)throws Exception{
  Address a=toAddr(start);disassemble(a);
  InstructionIterator it=currentProgram.getListing().getInstructions(a,true);
  while(it.hasNext()){
   Instruction ins=it.next();long x=ins.getAddress().getOffset();if(x>=end)break;
   println(ins.getAddress()+"  "+ins);
   for(Reference r:ins.getReferencesFrom())println("  REF "+r.getReferenceType()+" -> "+r.getToAddress());
  }
 }
 @Override public void run()throws Exception{
  println("=== STALK 0046AA50 ===");dump(0x0046AA60L,0x0046AAD0L);
  println("=== OTHER USER ===");dump(0x0046C000L,0x0046C200L);
 }
}