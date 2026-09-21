import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
public class CurrentAttackerWatchBotAssemblyEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  Address s=toAddr(0x00455660L);disassemble(s);
  InstructionIterator it=currentProgram.getListing().getInstructions(s,true);
  int n=0;
  while(it.hasNext()&&n<110){
   Instruction ins=it.next();
   if(ins.getAddress().getOffset()>=0x00455768L)break;
   println(ins.getAddress()+"  "+ins);
   n++;
  }
 }
}