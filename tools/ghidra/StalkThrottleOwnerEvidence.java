import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.Reference;
public class StalkThrottleOwnerEvidence extends GhidraScript {
 @Override public void run()throws Exception{
  Address a=toAddr(0x0046AA20L);disassemble(a);
  InstructionIterator it=currentProgram.getListing().getInstructions(a,true);
  while(it.hasNext()){Instruction ins=it.next();long x=ins.getAddress().getOffset();if(x>=0x0046AA65L)break;
   println(ins.getAddress()+"  "+ins);for(Reference r:ins.getReferencesFrom())println("  REF "+r.getReferenceType()+" -> "+r.getToAddress());
  }
 }
}